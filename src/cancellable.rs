struct CancellationTokenInner {
  request_id: Id,
  is_canceled: AtomicBool,
  client: Client,
}

#[derive(Clone)]
pub struct CancellationToken {
  inner: Arc<CancellationTokenInner>,
}

impl CancellationToken {
  pub fn is_canceled(&self) -> bool {
    self.inner.is_canceled.load(Ordering::SeqCst)
  }

  pub async fn cancel(self)  {
    if self.inner.is_canceled.swap(true, Ordering::SeqCst) {
      // Canceled already, should not send another notification.
      return;
    }

    self.inner
      .client
      .send_notification::<Cancel>(CancelParams {
        cancellation_reason: Some("RequestCancelled".to_string()),
        completions: vec![]
      })
      .await;
  }
}

