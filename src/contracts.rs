use anyhow::{Context, Result, bail};
use jsonschema::Registry;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub enum Contract {
    Run,
    Report,
    Plan,
    CommandResult,
}

pub fn validate<T: Serialize>(contract: Contract, value: &T) -> Result<()> {
    let instance = serde_json::to_value(value).context("failed to serialize contract value")?;
    let schema_document = schema(contract)?;
    let validator = match contract {
        Contract::Report => {
            let run_schema = schema(Contract::Run)?;
            let registry = Registry::new()
                .add(
                    "https://github.com/egohygiene/optiflow/schemas/run.schema.json",
                    run_schema,
                )?
                .prepare()?;
            jsonschema::options()
                .with_registry(&registry)
                .build(&schema_document)
                .context("failed to compile report schema")?
        }
        _ => {
            jsonschema::validator_for(&schema_document).context("failed to compile JSON Schema")?
        }
    };
    if let Err(error) = validator.validate(&instance) {
        bail!(
            "contract validation failed at {}: {error}",
            error.instance_path()
        );
    }
    Ok(())
}

pub fn schema(contract: Contract) -> Result<Value> {
    let source = match contract {
        Contract::Run => include_str!("../schemas/run.schema.json"),
        Contract::Report => include_str!("../schemas/report.schema.json"),
        Contract::Plan => include_str!("../schemas/plan.schema.json"),
        Contract::CommandResult => include_str!("../schemas/command-result.schema.json"),
    };
    serde_json::from_str(source).context("checked-in JSON Schema is invalid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn result(class: &str, exit_code: u64) -> Value {
        json!({
            "schema": "optiflow.command-result.v1",
            "command": "scan",
            "outcome": { "class": class, "exit_code": exit_code },
            "artifacts": [],
            "diagnostics": [],
            "result": null
        })
    }

    #[test]
    fn command_result_schema_accepts_every_stable_outcome_mapping() {
        let mut success = result("success", 0);
        success["coverage"] = json!({ "status": "complete" });
        success["result"] = json!({});

        let mut partial = result("partial_success", 3);
        partial["coverage"] = json!({ "status": "partial" });
        partial["artifacts"] = json!([{
            "kind": "report",
            "schema": "optiflow.report.v3",
            "run_id": "018f47a2-4f17-7b00-8000-000000000000",
            "path": { "encoding": "utf8", "value": "/tmp/report.json" }
        }]);

        for document in [
            success,
            result("internal_failure", 1),
            result("invalid_input", 2),
            partial,
            result("capability_unavailable", 4),
            result("stale_state", 5),
            result("interrupted", 130),
            result("terminated", 143),
        ] {
            validate(Contract::CommandResult, &document).expect("valid command result");
        }
    }

    #[test]
    fn command_result_schema_rejects_contradictory_outcomes() {
        let cases = [
            result("success", 3),
            {
                let mut value = result("success", 0);
                value["coverage"] = json!({ "status": "partial" });
                value
            },
            {
                let mut value = result("partial_success", 3);
                value["coverage"] = json!({ "status": "partial" });
                value
            },
            result("unknown", 0),
        ];

        for document in cases {
            assert!(validate(Contract::CommandResult, &document).is_err());
        }
    }

    #[test]
    fn command_result_schema_closes_diagnostics_and_native_paths() {
        let mut invalid_diagnostic = result("invalid_input", 2);
        invalid_diagnostic["diagnostics"] = json!([{
            "code": "not_a_public_code",
            "severity": "error",
            "classification": "input",
            "impact": "blocks_command",
            "message": "invalid"
        }]);
        assert!(validate(Contract::CommandResult, &invalid_diagnostic).is_err());

        let mut invalid_path = result("partial_success", 3);
        invalid_path["coverage"] = json!({ "status": "partial" });
        invalid_path["artifacts"] = json!([{
            "kind": "report",
            "schema": "optiflow.report.v3",
            "path": { "encoding": "locale_text", "value": "/tmp/report.json" }
        }]);
        assert!(validate(Contract::CommandResult, &invalid_path).is_err());
    }
}
