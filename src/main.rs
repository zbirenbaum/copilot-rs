mod utils;
mod auth;

fn main() {
  let mac_addr = utils::get_mac_addr();
  let device_auth = auth::device_auth();
  println!("MAC address: {}", mac_addr);
}
