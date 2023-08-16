use tower::Service;
use tower::Layer;
use std::future::Future;
use std::task::{Context, Poll};
use std::time::Duration;
use std::pin::Pin;
use std::fmt;
use std::error::Error;

// Our timeout service, which wraps another service and
// adds a timeout to its response future.
pub struct Timeout<T> {
  inner: T,
  timeout: Duration,
}

impl<T> Timeout<T> {
  pub fn new(inner: T, timeout: Duration) -> Timeout<T> {
    Timeout {
      inner,
      timeout
    }
  }
}

// The error returned if processing a request timed out
#[derive(Debug)]
pub struct Expired;

impl fmt::Display for Expired {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "expired")
  }
}

impl Error for Expired {}

// We can implement `Service` for `Timeout<T>` if `T` is a `Service`
impl<T, Request> Service<Request> for Timeout<T>
where
T: Service<Request>,
T::Future: 'static,
T::Error: Into<Box<dyn Error + Send + Sync>> + 'static,
T::Response: 'static,
{
  // `Timeout` doesn't modify the response type, so we use `T`'s response type
  type Response = T::Response;
  // Errors may be either `Expired` if the timeout expired, or the inner service's
  // `Error` type. Therefore, we return a boxed `dyn Error + Send + Sync` trait object to erase
  // the error's type.
  type Error = Box<dyn Error + Send + Sync>;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

  fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    // Our timeout service is ready if the inner service is ready.
    // This is how backpressure can be propagated through a tree of nested services.
    self.inner.poll_ready(cx).map_err(Into::into)
  }

  fn call(&mut self, req: Request) -> Self::Future {
    // Create a future that completes after `self.timeout`
    let timeout = tokio::time::sleep(self.timeout);

    // Call the inner service and get a future that resolves to the response
    let fut = self.inner.call(req);

    // Wrap those two futures in another future that completes when either one completes
    // If the inner service is too slow the `sleep` future will complete first
    // And an error will be returned and `fut` will be dropped and not polled again
    //
    // We have to box the errors so the types match
    let f = async move {
      tokio::select! {
        res = fut => {
          res.map_err(|err| err.into())
        },
        _ = timeout => {
          Err(Box::new(Expired) as Box<dyn Error + Send + Sync>)
        },
      }
    };

    Box::pin(f)
  }
}

// A layer for wrapping services in `Timeout`
pub struct TimeoutLayer(Duration);

impl TimeoutLayer {
  pub fn new(delay: Duration) -> Self {
    TimeoutLayer(delay)
  }
}

impl<S> Layer<S> for TimeoutLayer {
  type Service = Timeout<S>;

  fn layer(&self, service: S) -> Timeout<S> {
    Timeout::new(service, self.0)
  }
}
