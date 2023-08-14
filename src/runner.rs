use std::time::{Duration, Instant};
use std::thread;
use futures::future::{Abortable, AbortHandle, Aborted};
use std::sync::{Arc, Mutex, atomic::{Ordering, AtomicBool, AtomicU16}};
use futures::future::{Abortable, AbortHandle, Aborted, FutureExt, Future, LocalBoxFuture};



#[tower_lsp::async_trait(?Send)]
trait Runner: Send + Sync {
  type ReturnType: Send; // associated type
  async fn run(&self) -> Option<Self::ReturnType>;
}

type Context = ();
type AsyncCb<'r> = Box<dyn FnOnce(&'r Context) -> LocalBoxFuture<'r, ()> + 'r>;

struct StateManager<AsyncCb> {
  pending_cb: Box<AsyncCb>
}

impl StateManager {
  pub fn new()
  fn on_execute() {

  }
  fn normalize_async_cb<'r, Fut: Future<Output = ()> + 'r>(
    f: fn(&'r Context) -> Fut,
  ) -> impl FnOnce(&'r Context) -> LocalBoxFuture<'r, ()> {
    let cb = move |ctx: &'r Context| f(ctx).boxed_local();
    cb
  }
}

type ReturnType = impl Future<Output =  Params>;
struct Params { a: u32, b: String }
fn get_callback(p: Params) -> impl Future<Output =  Params> { 
  async { p }
}

#[test]
fn test_params() {
  let params = Params { a: 1, b: "Hello".to_string() };
  let manager = StateManager<ReturnType> {
    get_callback(params);
  };
}
let (abort_handle, abort_registration) = AbortHandle::new_pair();
let future = Abortable::new(async { 2 }, abort_registration);

abort_handle.abort();
assert_eq!(future.await, Err(Aborted));
//
//
// fn do_async<T: Runner, P: Param>(f: &'static T, params: P) -> impl FnOnce()-> Option<T::ReturnType> {
//   let (sender, receiver) = channel::<Option<T::ReturnType>>();
//   let hand = thread::spawn(move || {
//     sender.send(f.run()).unwrap(); 
//   });
//   let f = move || -> Option<T::ReturnType> {
//     let res = receiver.recv().unwrap();
//     hand.join().unwrap();
//     return res;
//   };
//   return f;
// }
//
// struct Calculation {  // <---- choose: name
//   value: i32  // <----- choose: inputs for your async work
// }
//
// impl Runner for Calculation {
//   type ReturnType = String;  // <--- choose: calculation return type
//   async fn run(&self) -> Option<Self::ReturnType> {  // <-- implement: code executed by a thread
//     println!("async calculation starts");
//     thread::sleep(Duration::from_millis(3000));
//
//     return Some(self.value * 2);
//   }
// }
// #[test]
// async fn test() {
//     let fut = do_async(&Calculation{ value: 12 });
//
//     let resp = fut().unwrap(); // call fut() to wait for the respbnse
//
//     println!("{}", resp);
// }
