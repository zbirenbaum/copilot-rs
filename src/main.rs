mod utils;
mod auth;
mod reader;

fn main() {
  let mac_addr = utils::get_mac_addr();
  // let device_auth = auth::device_auth();
  let (user, token) = reader::read_config();
  println!("{:?}", user);
  println!("{:?}", token);
  println!("MAC address: {}", mac_addr);
}
