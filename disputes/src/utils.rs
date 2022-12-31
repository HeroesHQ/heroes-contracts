use crate::*;

impl DisputesContract {
  pub(crate) fn nano_to_sec(timestamp: Timestamp) -> TimestampSec {
    (timestamp / 10u64.pow(9)) as _
  }

  pub(crate) fn chunk_of_description(dispute: &Dispute, reason: &Reason) -> String {
    let mut chunk = "".to_string().to_owned();
    let (side_str, account_id) = match reason.side {
      Side::Claimer => ("Claimer", dispute.claimer.clone()),
      _ => ("Project owner", dispute.project_owner_delegate.clone()),
    };
    chunk.push_str("\n\n");
    chunk.push_str(Self::nano_to_sec(reason.argument_timestamp.into()).to_string().as_str());
    chunk.push_str(format!(" {} ({}):", side_str, account_id).as_str());
    chunk.push_str("\n");
    chunk.push_str(&reason.description);
    chunk
  }
}
