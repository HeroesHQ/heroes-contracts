use crate::*;

#[near_bindgen]
impl DisputesContract {
  pub(crate) fn timestamp_to_text_date(time: U64) -> String {
    let timestamp = time.0 / 1_000_000_000;
    let naive = NaiveDateTime::from_timestamp_opt(timestamp as i64, 0);
    let datetime: DateTime<Utc> = DateTime::from_utc(naive.unwrap(), Utc);
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
  }
}
