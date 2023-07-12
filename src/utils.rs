use interfaces::Interface;
use std::process;
use sha256::digest;

pub fn get_mac_addr() -> String {
  fn try_get_mac_addr() -> Result<String,()> {
    fn validate_interface(itf: &Interface) -> bool {
      let exclude = vec!["00:00:00:00:00:00", "ff:ff:ff:ff:ff:ff", "ac:de:48:00:11:22"];
      return itf.is_running() &&
        !exclude.iter().any(|&s| s == itf.hardware_addr()
          .unwrap()
          .as_string()
        );
    }
    let ifaces = Interface::get_all().unwrap();
    let itf = ifaces.iter().find(|&itf| validate_interface(itf));
    match itf {
      Some(x) => Ok(
        x.hardware_addr()
          .unwrap()
          .to_string()),
      None => Err(())
    }
  }
  let mac_addr = try_get_mac_addr();
  if mac_addr.is_err() {
    eprintln!("Problem finding hardware address");
    process::exit(1);
  }
  digest(mac_addr.unwrap())
}
