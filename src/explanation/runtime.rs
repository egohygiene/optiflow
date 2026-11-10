//! Explicit local-process handoff through the existing SDK v1 reporting role.
//! A trusted process is not an OS sandbox or proof of model locality/quality.
use super::*;
use crate::extensions::*;
use uuid::Uuid;
pub const PROFILE_SCHEMA:&str="optiflow.evidence-explanation-runtime-profile.v1";
pub const CONFIG_SCHEMA:&str="optiflow.evidence-explanation-runtime-config.v1";
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema:String,pub fingerprint:Digest,pub provider_id:String,pub model_id:String,
    pub runtime_revision:String,pub template:String,pub template_fingerprint:Digest,
    pub output_schema_digest:Digest,pub backend:String,pub local_only:bool,
}
impl Profile {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{ensure!(bytes.len()<=16_384,"runtime profile exceeds 16 KiB");let raw:serde_json::Value=serde_json::from_slice(bytes)?;contracts::validate(Contract::ExplanationRuntimeProfile,&raw)?;let p:Self=serde_json::from_slice(bytes)?;p.validate()?;Ok(p)}
    pub fn validate(&self)->Result<()> {
        let mut copy=self.clone();copy.fingerprint=Digest::blake3(b"");
        ensure!(self.schema==PROFILE_SCHEMA&&self.fingerprint==fingerprint(&copy)?&&self.backend=="local_process_sdk_v1"&&self.local_only
            &&!self.template.is_empty()&&self.template.len()<=4096&&!self.template.chars().any(|c|c.is_control()&&c!='\n')
            &&self.template_fingerprint==Digest::blake3(self.template.as_bytes())
            &&self.output_schema_digest==Digest::blake3(include_bytes!("../../schemas/explanation-candidate-v1.schema.json")),"runtime profile/template/output identity differs");
        for name in [&self.provider_id,&self.model_id,&self.runtime_revision]{ensure!(!name.is_empty()&&name.len()<=128&&name.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-:/+".contains(&b)),"bounded declared runtime identity required");}Ok(())
    }
    pub(super) fn identity(&self)->DeclaredIdentity {DeclaredIdentity {provider_id:self.provider_id.clone(),model_id:self.model_id.clone(),runtime_revision:self.runtime_revision.clone(),profile_fingerprint:self.fingerprint.clone(),template_fingerprint:self.template_fingerprint.clone(),output_schema_digest:self.output_schema_digest.clone()}}
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfiguration {pub enabled:bool,pub profile:Profile}
/// The SDK owns executable/manifest/lock/result validation and bounded argv
/// transport. Only normalized facts and the exact locked profile cross stdin.
pub fn prepare_invocation(extension:&LoadedExtension,profile:&Profile,packet:&Packet,cancel:&impl Fn()->bool)->Result<Option<ExtensionInvocationV1>>{
    ensure!(!cancel(),"explanation runtime cancelled");profile.validate()?;packet.validate()?;
    let lock=extension.lock();
    let selected:Vec<_>=[CONFIG_SCHEMA,super::ollama::CONFIG_SCHEMA].into_iter().filter_map(|id|lock.configurations.get(id).map(|value|(id,value))).collect();
    ensure!(selected.len()==1,"select exactly one locked explanation runtime configuration");let (configuration_schema,locked)=selected[0];
    let (enabled,effects,network_hosts)=if configuration_schema==CONFIG_SCHEMA {
        contracts::validate(Contract::ExplanationRuntimeConfig,&locked.values)?;
        let c:RuntimeConfiguration=serde_json::from_value(locked.values.clone())?;ensure!(c.profile==*profile,"runtime profile differs from operator lock");
        (c.enabled,BTreeSet::from([ExtensionEffect::Subprocess]),Vec::<String>::new())
    }else{
        contracts::validate(Contract::ExplanationOllamaConfig,&locked.values)?;
        let c:super::ollama::Configuration=serde_json::from_value(locked.values.clone())?;c.validate()?;ensure!(c.profile==*profile,"local runtime profile differs from operator lock");
        (c.enabled,BTreeSet::from([ExtensionEffect::Subprocess,ExtensionEffect::Network]),vec![c.deployment.endpoint.to_string()])
    };
    ensure!(locked.fingerprint==fingerprint(&locked.values)?,"runtime configuration fingerprint differs");
    if !enabled {return Ok(None);}
    ensure!(lock.enabled&&lock.trust==TrustMode::TrustedProcess&&lock.granted_effects==effects
        &&lock.allowed_replacements.is_empty()&&lock.allowed_fallbacks.is_empty(),"explicit trusted local process and subprocess-only grants required; no fallbacks");
    let capability=extension.manifest().capabilities.iter().find(|c|c.capability_id=="optiflow/report/evidence-explanation-runtime").context("runtime report capability absent")?;
    ensure!(capability.kind==CapabilityKind::ReportProvider&&capability.configuration_schema==configuration_schema
        &&capability.accepts==[PACKET_SCHEMA]&&capability.produces==[CANDIDATE_SCHEMA]
        &&capability.effects==effects&&!capability.content_changes&&capability.locality.local_only
        &&capability.locality.network_hosts==network_hosts,"runtime capability must declare normalized reporting and local-only subprocess effects");
    let Some(ExecutionModeDeclaration::Process {limits,..})=extension.selected_execution_mode() else {anyhow::bail!("explicit SDK process mode required")};
    ensure!((1..=30_000).contains(&limits.timeout_ms)&&(1..=131_072).contains(&limits.max_stdin_bytes)
        &&(1..=131_072).contains(&limits.max_stdout_bytes)&&(1..=4096).contains(&limits.max_stderr_bytes),"runtime process bounds exceed explanation profile");
    if configuration_schema==super::ollama::CONFIG_SCHEMA {
        let Some(ExecutionModeDeclaration::Process {name,arguments,..})=extension.selected_execution_mode() else {anyhow::bail!("local SDK process mode required")};
        ensure!(name=="ollama-sdk-v1"&&arguments==&vec![super::ollama::MARKER.to_string()]&&capability.capability_version=="1.0.0","fixed local provider mode/arguments/version required");
    }
    let value=serde_json::to_value(packet)?;let input_fingerprint=fingerprint(&value)?;
    let invocation=ExtensionInvocationV1 {schema:EXTENSION_INVOCATION_SCHEMA.into(),invocation_id:Uuid::now_v7().to_string(),extension:ExtensionIdentity {extension_id:extension.manifest().extension_id.clone(),version:extension.manifest().version.clone(),manifest_digest:extension.manifest_digest().clone()},capability_id:capability.capability_id.clone(),capability_kind:CapabilityKind::ReportProvider,input_artifacts:vec![ImmutableArtifactReference {artifact_id:"normalized-records".into(),schema:PACKET_SCHEMA.into(),content_digest:input_fingerprint.clone()}],input_evidence:vec![EvidenceContribution {evidence_id:"normalized-records".into(),schema:PACKET_SCHEMA.into(),fingerprint:input_fingerprint,value}],configuration:InvocationConfiguration {schema_id:configuration_schema.into(),values:locked.values.clone(),fingerprint:locked.fingerprint.clone()},authorization:AuthorizationEvidence {lock_id:lock.lock_id.clone(),lock_digest:extension.lock_digest().clone(),grants_digest:lock.grants_digest.clone(),granted_effects:effects},limits:*limits,cancellation_id:Uuid::now_v7().to_string(),checkpoint_references:vec![],lifecycle_event:None};
    ensure!(serde_json::to_vec(&invocation)?.len() as u64<=limits.max_stdin_bytes,"normalized invocation exceeds locked stdin bound");Ok(Some(invocation))
}
pub struct RuntimeOutput {pub candidate:Candidate,pub sdk_result:ExtensionResultV1}
fn accepted(profile:&Profile,packet:&Packet,result:&ExtensionResultV1,allowed_effects:&BTreeSet<ExtensionEffect>,cancel:&impl Fn()->bool)->Result<Candidate>{
    ensure!(!cancel()&&result.outcome==ExtensionOutcome::Produced&&result.reports.len()==1&&result.reports[0].schema==CANDIDATE_SCHEMA
        &&result.policy_contributions.is_empty()&&result.plan_operations.is_empty()&&result.validations.is_empty()
        &&result.evidence.is_empty()&&result.checkpoint_references.is_empty()
        &&result.observed_effects.is_subset(allowed_effects),"runtime result exceeds advisory reporting boundary");
    let report=&result.reports[0];ensure!(report.fingerprint==fingerprint(&report.value)?,"runtime report fingerprint changed");
    let candidate=Candidate::from_json_slice(&serde_json::to_vec(&report.value)?)?;
    ensure!(candidate.declared_identity==profile.identity(),"returned model/runtime/profile/template identity differs from explicit lock");
    validate_candidate(packet,&candidate,cancel)?;Ok(candidate)
}
/// Starts exactly one operator-locked SDK process when explicitly invoked.
/// No PATH discovery, shell, inherited environment, credential input, model
/// download, provider installation, router, retries or remote fallback.
pub fn invoke(extension:LoadedExtension,profile:&Profile,packet:&Packet,cancel:&impl Fn()->bool)->Result<Option<RuntimeOutput>>{
    let Some(invocation)=prepare_invocation(&extension,profile,packet,cancel)? else {return Ok(None);};
    let client=ProcessExtensionClient::new(extension)?;
    let result=client.invoke(&invocation,cancel)?;
    let candidate=accepted(profile,packet,&result,&invocation.authorization.granted_effects,cancel)?;
    Ok(Some(RuntimeOutput {candidate,sdk_result:result}))
}
#[cfg(test)] mod tests;
