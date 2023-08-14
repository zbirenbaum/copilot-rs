// use std::time::{Duration, Instant};
// use std::thread::{self, JoinHandle};
// use futures::future::{Abortable, AbortHandle, Aborted};
// use std::sync::{Arc, Mutex, atomic::{Ordering, AtomicBool, AtomicU16}};
//
// type Func<T> = Box<dyn FnMut() + Send + Sync>;
//
// #[derive(Clone)]
// pub struct Container<T> {
//   pub func: Arc<Mutex<Func>>,
//   pub pending: Arc<AtomicBool>,
//   pub last_instant: Instant,
//   pub last_thread: Mutex<Option<JoinHandle<T>>>
// }
//
// impl Container {
//   pub fn new(func: Func) -> Self {
//     let (abort_handle, abort_registration) = AbortHandle::new_pair();
//     Container {
//       func: Arc::new(Mutex::new(func)),
//       last_thread: None,
//       pending: Arc::new(AtomicBool::new(false)),
//       last_instant: Instant::now()
//     }
//   }
//
//   pub fn create_task(&self, func: Func) {
//     let mut last_thread = self.last_thread.lock();
//     *last_thread = thread::spawn(async move {
//
//     });
//     let mut lock = self.func.lock().unwrap();
//     *lock = func;
//   }
//
//   pub fn execute(&self) {
//     let mut last_thread = self.last_thread.lock().unwrap();
//     if last_thread.is_none() {
//
//     }
//     let mut lock = self.func.lock().unwrap();
//     (lock)();
//   }
//
//   pub fn update(&self, func: Func) {
//     let mut lock = self.func.lock().unwrap();
//     *lock = func;
//   }
// }
//
// type Callback<T> = Box<dyn Fn(T) + Send + Sync>;
//
// struct StateManager<T> {
//   pub has_pending: Arc<AtomicBool>,
//   pub has_result: Arc<AtomicBool>,
//   pub handles: Arc<Vec<thread::JoinHandle<T>>>,
//   pub result: Arc<Option<T>>
// }
//
// impl StateManager {
//   pub fn new() -> Self {
//     let has_result= Arc::new(AtomicBool::new(false));
//     let received_new = Arc::new(AtomicBool::new(false));
//     Self {
//       has_pending: Arc::new(AtomicBool::new(false)),
//       has_result: Arc::new(AtomicBool::new(false)),
//       handles: Arc::new(vec![]),
//       result: None
//     }
//   }
//
//   pub fn set_pending(&self, state: bool) {
//     let has_pending = Arc::clone(&self.has_pending);
//     has_pending.store(state, Ordering::AcqRel)
//   }
//
//   pub fn set_newer(&self, state: bool) {
//     let has_newer = Arc::clone(&self.has_newer);
//     has_newer.store(state, Ordering::AcqRel)
//   }
//
//   pub fn execute(&self, cb: Func) {
//     let has_pending = Arc::clone(&self.has_pending);
//     if has_pending.load(Ordering::AcqRel) { self.handles[0] }
//   }
// }
//
// #[test]
// fn debounce() {
//   let meow = || {
//     println!("meow");
//   };
//   let woof = || {
//     println!("woof");
//   };
//   let mut shared_container = Container::new(Box::new(meow));
//   let mut shared_container_clone = shared_container.clone();
//
//   let has_pending = Arc::new(AtomicBool::new(false));
//   let has_pending_clone = Arc::clone(&has_pending);
//
//   let received_new = Arc::new(AtomicBool::new(false));
//   let has_pending_clone = Arc::clone(&received_new);
//   let handle = thread::spawn(move || {
//     // We want to wait until the flag is set. We *could* just spin, but using
//     // park/unpark is more efficient.
//     while !flag2.load(Ordering::Acquire) {
//       println!("Parking thread");
//       thread::park();
//       // We *could* get here spuriously, i.e., way before the 10ms below are over!
//       // But that is no problem, we are in a loop until the flag is set anyway.
//       println!("Thread unparked");
//     }
//     thread::sleep(Duration::from_millis(200));
//     // during sleep period a new req was sent in
//     if !flag2.load(Ordering::Acquire) {
//       return "false"
//     }
//     println!("Flag received");
//     shared_container_clone.execute();
//     return "true";
//   });
//
//   shared_container.update(Box::new(woof));
//   let res = handle.join().unwrap();
//   flag.store(false, Ordering::Release);
// }
//
