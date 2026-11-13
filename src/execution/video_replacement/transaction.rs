//! A distinct restorable video action reuses the replacement lock/journal and
//! namespace engine. PNG limits, approvals and legacy event meanings are intact.
use super::super::replacement::transaction::{
    self as engine, ReplacementEvent, TransactionBody, TransactionLimits, TransactionView,
    VideoAction,
};
use super::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanBody {
    pub operation: String,
    pub validation: live::Request,
    pub free_space_reserve_bytes: u64,
    pub journal_budget_bytes: u64,
    pub original_disposition: String,
    pub qualification: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub fingerprint: String,
    pub body: PlanBody,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub schema: String,
    pub authorization_id: String,
    pub plan_fingerprint: String,
    pub authority: String,
    pub approved_by: String,
    pub approved_at: String,
}
impl Plan {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= 4_194_304,
            "video replacement plan exceeds 4 MiB"
        );
        let raw: serde_json::Value = serde_json::from_slice(bytes)?;
        crate::contracts::validate(crate::contracts::Contract::VideoReplacementPlan, &raw)?;
        let p: Self = serde_json::from_slice(bytes)?;
        p.validate()?;
        Ok(p)
    }
    pub fn validate(&self) -> Result<()> {
        self.body.validation.validate()?;
        let b = &self.body;
        ensure!(
            self.schema == "optiflow.video-replacement-plan.v1"
                && self.fingerprint == hash(b)?
                && b.operation == "replace_validated_video_stream_copy.v1"
                && b.original_disposition == "retain_for_restore"
                && b.qualification == "unqualified_source_checkpoint"
                && (1..=1_099_511_627_776).contains(&b.free_space_reserve_bytes)
                && (65_536..=16_777_216).contains(&b.journal_budget_bytes),
            "video replacement plan identity/retention/bounds differ"
        );
        Ok(())
    }
}
impl Approval {
    pub fn from_json_slice(bytes: &[u8], plan: &Plan) -> Result<Self> {
        ensure!(bytes.len() <= 16_384, "video approval exceeds 16 KiB");
        let raw: serde_json::Value = serde_json::from_slice(bytes)?;
        crate::contracts::validate(crate::contracts::Contract::VideoReplacementApproval, &raw)?;
        let a: Self = serde_json::from_slice(bytes)?;
        a.validate(plan)?;
        Ok(a)
    }
    pub fn validate(&self, plan: &Plan) -> Result<()> {
        plan.validate()?;
        let mut copy = self.clone();
        copy.authorization_id.clear();
        ensure!(
            self.schema == "optiflow.video-replacement-approval.v1"
                && self.authorization_id == hash(&copy)?
                && self.plan_fingerprint == plan.fingerprint
                && self.authority == "restorable_video_replacement_only"
                && !self.approved_by.trim().is_empty()
                && self.approved_by.len() <= 128
                && !self.approved_by.chars().any(char::is_control)
                && chrono::DateTime::parse_from_rfc3339(&self.approved_at).is_ok(),
            "video replacement approval differs"
        );
        Ok(())
    }
}
pub fn prepare(validation: live::Request, reserve: u64, journal: u64) -> Result<Plan> {
    let body = PlanBody {
        operation: "replace_validated_video_stream_copy.v1".into(),
        validation,
        free_space_reserve_bytes: reserve,
        journal_budget_bytes: journal,
        original_disposition: "retain_for_restore".into(),
        qualification: "unqualified_source_checkpoint".into(),
    };
    let p = Plan {
        schema: "optiflow.video-replacement-plan.v1".into(),
        fingerprint: hash(&body)?,
        body,
    };
    p.validate()?;
    Ok(p)
}
pub fn approve(plan: &Plan, approved_by: String) -> Result<Approval> {
    let mut a = Approval {
        schema: "optiflow.video-replacement-approval.v1".into(),
        authorization_id: String::new(),
        plan_fingerprint: plan.fingerprint.clone(),
        authority: "restorable_video_replacement_only".into(),
        approved_by,
        approved_at: chrono::Utc::now().to_rfc3339(),
    };
    a.authorization_id = hash(&a)?;
    a.validate(plan)?;
    Ok(a)
}
fn view(
    plan: &Plan,
    approval: &Approval,
    action: VideoAction,
    evidence: Option<serde_json::Value>,
) -> TransactionView {
    let r = &plan.body.validation.review;
    TransactionView {
        fingerprint: plan.fingerprint.clone(),
        authorization_id: approval.authorization_id.clone(),
        body: TransactionBody {
            source: r.source.clone(),
            candidate: r.candidate.clone(),
            state: r.state.clone(),
            retention: r.retention.clone(),
            limits: TransactionLimits {
                maximum_file_bytes: plan
                    .body
                    .validation
                    .production
                    .limits
                    .input_bytes
                    .max(plan.body.validation.production.limits.candidate_bytes),
                free_space_reserve_bytes: plan.body.free_space_reserve_bytes,
                journal_budget_bytes: plan.body.journal_budget_bytes,
            },
        },
        video_action: Some(action),
        media_validation: evidence,
    }
}
/// Refuses before native validation and writable journal access until the
/// existing native replacement qualification work is completed separately.
pub fn apply(plan: &Plan, approval: &Approval, signals: &SignalState) -> Result<ReplacementEvent> {
    approval.validate(plan)?;
    engine::require_qualification()?;
    #[cfg(target_os = "linux")]
    {
        let validated = live::validate(&plan.body.validation, signals)?;
        let evidence = serde_json::to_value(validated.record())?;
        let action = VideoAction {
            operation: plan.body.operation.clone(),
            review_fingerprint: plan.body.validation.review.fingerprint.clone(),
            validation_request_fingerprint: hash(&plan.body.validation)?,
            current_validation_fingerprint: hash(&evidence)?,
        };
        engine::apply_shared(
            &view(plan, approval, action, Some(evidence)),
            signals,
            &|| validated.recheck(signals),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = signals;
        anyhow::bail!("video replacement property backend is not qualified on this platform")
    }
}
pub fn status(plan: &Plan) -> Result<Vec<ReplacementEvent>> {
    plan.validate()?;
    let records = engine::status_for(
        &plan.body.validation.review.state,
        &plan.fingerprint,
        plan.body.journal_budget_bytes,
    )?;
    for e in &records {
        let a = e
            .video_action
            .as_ref()
            .context("video plan cannot consume legacy PNG events")?;
        ensure!(
            a.operation == plan.body.operation
                && a.review_fingerprint == plan.body.validation.review.fingerprint
                && a.validation_request_fingerprint == hash(&plan.body.validation)?,
            "video event action differs from exact plan"
        );
    }
    Ok(records)
}
pub fn restore(plan: &Plan, approval: &Approval, signals: &SignalState) -> Result<()> {
    approval.validate(plan)?;
    engine::require_qualification()?;
    #[cfg(target_os = "linux")]
    {
        let records = status(plan)?;
        let action = records
            .last()
            .and_then(|e| e.video_action.clone())
            .context("committed video action absent")?;
        engine::restore_shared(&view(plan, approval, action, None), signals)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = signals;
        anyhow::bail!("video restore property backend unavailable")
    }
}
