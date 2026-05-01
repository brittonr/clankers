use crate::cli::BatchAction;
use crate::cli::TrajectoryFormatArg;
use crate::commands::CommandContext;
use crate::error::Error;
use crate::error::Result;
use crate::modes::batch::BatchRunConfig;
use crate::modes::batch::TrajectoryFormat;

pub async fn run(_ctx: &CommandContext, action: BatchAction) -> Result<()> {
    match action {
        BatchAction::Run {
            input,
            output,
            concurrency,
            format,
            resume,
        } => {
            let format = match format {
                TrajectoryFormatArg::Jsonl => TrajectoryFormat::Jsonl,
                TrajectoryFormatArg::Sharegpt => TrajectoryFormat::Sharegpt,
            };
            let config = BatchRunConfig::new(input, output, concurrency, format, resume);
            config.validate().map_err(|err| Error::Config {
                message: err.to_string(),
            })?;
            Err(Error::Agent {
                message: "batch execution backend is not wired yet; parsing and policy validation are available"
                    .to_string(),
            })
        }
    }
}
