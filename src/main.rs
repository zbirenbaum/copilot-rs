mod utils;

fn main() {
  let mac_addr = utils::get_mac_addr();
  println!("MAC address: {}", mac_addr);
}
