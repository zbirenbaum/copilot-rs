use cfgrammar::yacc;
use lrpar::RTParserBuilder;
use ouroboros::self_referencing;
use std::borrow::Cow;
use tower_lsp::jsonrpc;
use tower_lsp::lsp_types as lsp_ty;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use std::ops::DerefMut as _;

#[derive(thiserror::Error, Debug)]
enum ServerError {
    #[error("argument requires a path")]
    RequiresPath,
    #[error("Unknown argument")]
    UnknownArgument,
    #[error("Toml deserialization error")]
    TomlDeserialization(#[from] toml::de::Error),
    #[error("Json serialization error")]
    JsonSerialization(#[from] serde_json::Error),
    #[error("Sync io error {0}")]
    IO(#[from] std::io::Error),
}

#[derive(Debug)]
pub enum StateGraphPretty {
    CoreStates,
    ClosedStates,
    CoreEdges,
    AllEdges,
}

struct Backend {
    client: Client,
    state: tokio::sync::Mutex<State>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceCfg {
    workspace: nimbleparse_toml::Workspace,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct ServerDocumentParams {
    cmd: String,
    path: String,
}

impl Backend {
    async fn get_server_document(
        &self,
        params: ServerDocumentParams,
    ) -> jsonrpc::Result<Option<String>> {
        let state = self.state.lock().await;
        if params.cmd == "generictree.cmd" {
            let path = std::path::PathBuf::from(&params.path);
            let parser_info = state.parser_for(&path);
            // FIXME
            Ok(None)
        } else if params.cmd.starts_with("stategraph_") && params.cmd.ends_with(".cmd") {
            let path = std::path::PathBuf::from(&params.path);
            let parser_info = state.find_parser_info(&path);
            let property = params
                .cmd
                .strip_prefix("stategraph_")
                .unwrap()
                .strip_suffix(".cmd")
                .unwrap();
            if let Some(parser_info) = parser_info {
                let pretty_printer = match property {
                    "core_states" => StateGraphPretty::CoreStates,
                    "closed_states" => StateGraphPretty::ClosedStates,
                    "core_edges" => StateGraphPretty::CoreEdges,
                    "all_edges" => StateGraphPretty::AllEdges,
                    _ => return Ok(None),
                };
                // FIXME
                Ok(None)
            } else {
                Ok(None)
            }
        } else if params.cmd.starts_with("railroad.svg") && params.cmd.ends_with(".cmd") {
            let path = std::path::PathBuf::from(&params.path);
            let parser_info = state.find_parser_info(&path);
            if let Some(parser_info) = parser_info {
                // FIXME
                Ok(None)
            } else {
                Ok(None)
            }
        } else {
            Err(jsonrpc::Error {
                code: jsonrpc::ErrorCode::InvalidParams,
                message: Cow::from("Unknown command name"),
                data: Some(serde_json::Value::String(params.cmd)),
            })
        }
    }
}

type Workspaces = std::collections::HashMap<std::path::PathBuf, WorkspaceCfg>;
type ParserId = usize;

#[derive(Debug, Clone)]
pub struct ParserInfo {
    id: ParserId,
    l_path: std::path::PathBuf,
    y_path: std::path::PathBuf,
    recovery_kind: lrpar::RecoveryKind,
    yacc_kind: yacc::YaccKind,
    extension: std::ffi::OsString,
    quiet: bool,
}

impl ParserInfo {
    fn is_lexer(&self, path: &std::path::Path) -> bool {
        self.l_path == path
    }
    fn is_parser(&self, path: &std::path::Path) -> bool {
        self.y_path == path
    }
    fn id(&self) -> ParserId {
        self.id
    }
}

use lrlex::LexerDef as _;
type LexerDef = lrlex::LRNonStreamingLexerDef<lrlex::DefaultLexerTypes>;

pub struct ParserData(
    Option<(
        LexerDef,
        yacc::YaccGrammar,
        lrtable::StateGraph<u32>,
        lrtable::StateTable<u32>,
    )>,
);

#[self_referencing(pub_extras)]
pub struct ParsingState {
    data: ParserData,
    #[borrows(data)]
    #[covariant]
    rt_parser_builders: Option<RTParserBuilder<'this, u32, lrlex::DefaultLexerTypes<u32>>>,
}

struct State {
    client_monitor: bool,
    extensions: std::collections::HashMap<std::ffi::OsString, ParserInfo>,
    toml: Workspaces,
    warned_needs_restart: bool,
    parsing_state: Vec<ParsingState>,
}

impl State {
    fn affected_parsers(&self, path: &std::path::Path, ids: &mut Vec<usize>) {
        if let Some(extension) = path.extension() {
            let id = self.extensions.get(extension).map(ParserInfo::id);
            // A couple of corner cases here:
            //
            // * The kind of case where you have foo.l and bar.y/baz.y using the same lexer.
            //    -- We should probably allow this case where editing a single file updates multiple parsers.
            // * The kind of case where you have a yacc.y for the extension .y, so both the extension
            //   and the parse_info have the same id.
            //    -- We don't want to run the same parser multiple times: remove duplicates.
            // In the general case, where you either change a .l, .y, or a file of the parsers extension
            // this will be a vec of one element.
            if let Some(id) = id {
                ids.push(id);
            }

            ids.extend(
                self.extensions
                    .values()
                    .filter(|parser_info| path == parser_info.l_path || path == parser_info.y_path)
                    .map(ParserInfo::id),
            );

            ids.sort_unstable();
            ids.dedup();
        }
    }

    /// Expects to be given a path to a parser, returns the parser info for that parser.
    fn find_parser_info(&self, parser_path: &std::path::Path) -> Option<&ParserInfo> {
        self.extensions
            .values()
            .find(|parser_info| parser_info.y_path == parser_path)
    }

    fn parser_for(&self, path: &std::path::Path) -> Option<&ParserInfo> {
        path.extension().and_then(|ext| self.extensions.get(ext))
    }
}

fn initialize_failed(reason: String) -> jsonrpc::Result<lsp_ty::InitializeResult> {
    Err(tower_lsp::jsonrpc::Error {
        code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32002),
        message: Cow::from(format!("Error during server initialization: {reason}")),
        data: None,
    })
}

#[tower_lsp::async_trait(?Send)]
impl LanguageServer for Backend {
    async fn initialize(
        &mut self,
        params: lsp_ty::InitializeParams,
    ) -> jsonrpc::Result<lsp_ty::InitializeResult> {
        self.client
            .log_message(lsp_ty::MessageType::LOG, "initializing...")
            .await;

        let caps = params.capabilities;
        if params.workspace_folders.is_none() || caps.workspace.is_none() {
            initialize_failed("requires workspace & capabilities".to_string())?;
        }

        if !caps
            .text_document
            .map_or(false, |doc| doc.publish_diagnostics.is_some())
        {
            initialize_failed("requires diagnostics capabilities".to_string())?;
        }

        let mut state = self.state.lock().await;

        // vscode only supports dynamic_registration
        // neovim supports neither dynamic or static registration of this yet.
        state.client_monitor = caps.workspace.map_or(false, |wrk| {
            wrk.did_change_watched_files.map_or(false, |dynamic| {
                dynamic.dynamic_registration.unwrap_or(false)
            })
        });

        let paths = params.workspace_folders.unwrap();
        let paths = paths
            .iter()
            .map(|folder| folder.uri.to_file_path().unwrap());
        state.toml.extend(paths.map(|workspace_path| {
            let toml_path = workspace_path.join("nimbleparse.toml");
            // We should probably fix this, to not be sync when we implement reloading the toml file on change...
            let toml_file = std::fs::read_to_string(toml_path).unwrap();
            let workspace: nimbleparse_toml::Workspace =
                toml::de::from_str(toml_file.as_str()).unwrap();
            (workspace_path, WorkspaceCfg { workspace })
        }));

        Ok(lsp_ty::InitializeResult {
            capabilities: lsp_ty::ServerCapabilities {
                text_document_sync: Some(lsp_ty::TextDocumentSyncCapability::Kind(
                    lsp_ty::TextDocumentSyncKind::INCREMENTAL,
                )),
                hover_provider: Some(lsp_ty::HoverProviderCapability::Simple(true)),
                completion_provider: Some(lsp_ty::CompletionOptions::default()),
                // Can't return this *and* register in the editor client because of vscode.
                // returning this doesn't seem to handle arguments, or work with commands
                // that can be activationEvents.  So this is intentionally None,
                // even though we provide commands.
                execute_command_provider: None,
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&mut self, params: lsp_ty::InitializedParams) {
        let mut state = self.state.lock().await;
        let state = state.deref_mut();
        let mut globs: Vec<lsp_ty::Registration> = Vec::new();
        if state.client_monitor {
            for WorkspaceCfg { workspace, .. } in state.toml.values() {
                for parser in workspace.parsers.get_ref() {
                    let glob = format!("**/*{}", parser.extension.get_ref());
                    let mut reg = serde_json::Map::new();
                    reg.insert(
                        "globPattern".to_string(),
                        serde_json::value::Value::String(glob),
                    );
                    let mut watchers = serde_json::Map::new();
                    let blah = vec![serde_json::value::Value::Object(reg)];
                    watchers.insert(
                        "watchers".to_string(),
                        serde_json::value::Value::Array(blah),
                    );

                    globs.push(lsp_ty::Registration {
                        id: "1".to_string(),
                        method: "workspace/didChangeWatchedFiles".to_string(),
                        register_options: Some(serde_json::value::Value::Object(watchers)),
                    });
                }
            }
            self.client
                .log_message(
                    lsp_ty::MessageType::LOG,
                    format!("registering! {:?}", globs.clone()),
                )
                .await;
        }

        /* The lsp_types and lsp specification documentation say to register this dynamically
         * rather than statically, I'm not sure of a good place we can register it besides here.
         * Unfortunately register_capability returns a result, and this notification cannot return one;
         * given that this has to manually match errors and can't use much in the way of ergonomics.
         */
        if state.client_monitor {
            if let Err(e) = self.client.register_capability(globs).await {
                self.client
                    .log_message(
                        lsp_ty::MessageType::ERROR,
                        format!(
                            "registering for {}: {}",
                            "workspace/didChangeWatchedFiles", e
                        ),
                    )
                    .await;
                panic!("{}", e);
            }
        }
        // construct extension lookup table
        {
            let extensions = &mut state.extensions;
            for (workspace_path, workspace_cfg) in (state.toml).iter() {
                let workspace = &workspace_cfg.workspace;
                for (id, parser) in workspace.parsers.get_ref().iter().enumerate() {
                    let l_path = workspace_path.join(parser.l_file.get_ref());
                    let y_path = workspace_path.join(parser.y_file.get_ref());
                    let extension = parser.extension.clone().into_inner();
                    // Want this to match the output of Path::extension() so trim any leading '.'.
                    let extension_str = extension
                        .strip_prefix('.')
                        .map(|x| x.to_string())
                        .unwrap_or(extension);
                    let extension = std::ffi::OsStr::new(&extension_str);
                    let parser_info = ParserInfo {
                        id,
                        l_path: workspace_path.join(l_path),
                        y_path: workspace_path.join(y_path),
                        recovery_kind: parser.recovery_kind,
                        yacc_kind: parser.yacc_kind,
                        extension: extension.to_owned(),
                        quiet: parser.quiet,
                    };

                    extensions.insert(extension.to_os_string(), parser_info.clone());
                }
            }
        }

        self.client
            .log_message(lsp_ty::MessageType::LOG, "initialized!")
            .await;
    }

    async fn shutdown(&mut self) -> jsonrpc::Result<()> {
        Ok(())
    }
}

fn run_server_arg() -> std::result::Result<(), ServerError> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .build()?;
    rt.block_on(async {
        log::set_max_level(log::LevelFilter::Info);
        let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
        let (service, socket) = tower_lsp::LspService::build(|client| Backend {
            state: tokio::sync::Mutex::new(State {
                toml: std::collections::HashMap::new(),
                warned_needs_restart: false,
                client_monitor: false,
                extensions: std::collections::HashMap::new(),
                parsing_state: Vec::new(),
            }),
            client,
        })
        .custom_method(
            "nimbleparse_lsp/get_server_document",
            Backend::get_server_document,
        )
        .finish();
        tower_lsp::Server::new(stdin, stdout, socket)
            .serve(service)
            .await;
        Ok(())
    })
}

fn handle_workspace_arg(path: &std::path::Path) -> std::result::Result<(), ServerError> {
    let cfg_path = if path.is_dir() {
        path.join("nimbleparse.toml")
    } else {
        path.to_path_buf()
    };
    let toml_file = std::fs::read_to_string(cfg_path)?;
    let workspace: nimbleparse_toml::Workspace = toml::de::from_str(toml_file.as_str())?;
    serde_json::to_writer(std::io::stdout(), &workspace)?;
    Ok(())
}

fn main() -> std::result::Result<(), ServerError> {
    let mut args = std::env::args();
    let argv_zero = &args.next().unwrap();
    let exec_file = std::path::Path::new(argv_zero)
        .file_name()
        .unwrap()
        .to_string_lossy();

    #[cfg(all(feature = "console", tokio_unstable))]
    console_subscriber::init();

    if let Some(arg) = args.next() {
        let arg = arg.trim();
        if arg == "--workspace" {
            if let Some(file) = args.next() {
                // Sync
                let path = std::path::PathBuf::from(&file);
                handle_workspace_arg(path.as_path())
            } else {
                Err(ServerError::RequiresPath)
            }
        } else if arg == "--server" {
            // Async
            run_server_arg()
        } else {
            Err(ServerError::UnknownArgument)
        }
    } else {
        println!("{exec_file} --workspace [path] | --server");
        Ok(())
    }
}
