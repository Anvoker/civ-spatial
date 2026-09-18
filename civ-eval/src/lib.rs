//! `civ-eval` — encoders, questions/solvers, scorer, models, and the runner. Builds on the
//! neutral `Board` from `civ-core`; knows nothing about Freeciv.

pub mod describe;
pub mod encoders;
pub mod generators;
pub mod json;
pub mod model;
pub mod prompt;
pub mod question;
pub mod rules;
pub mod runner;
pub mod scoring;
pub mod sink;
pub mod trace;

#[cfg(feature = "remote")]
pub mod remote;

#[cfg(feature = "remote")]
pub use remote::{Effort, OpenAiCompatible};

pub use describe::describe_region;
pub use encoders::{
    encoder_by_name, queryable_surface, region_window, run_tool, Encoder, Interactive,
    InteractiveOps, QueryableSurface, RawOps, RecoveredCity, RecoveredTile, RecoveredUnit,
    Referent, DEFAULT_ENCODERS, QUERYABLE_SURFACES, STATIC_ENCODERS, V1_ENCODERS,
};
pub use generators::{all_kinds, generate_questions, generate_questions_fogged, Difficulty};
pub use model::{
    ChatModel, ChatReply, Message, Model, OracleModel, Reply, Role, ToolCall, ToolDef, Usage,
};
pub use question::{solve, Answer, EvalItem, Question, Tier};
pub use runner::{
    board_block_tokens, run, run_interactive, run_oracle, summarize, InteractiveConfig, ResultRow,
};
pub use scoring::{score, Score, Status};
pub use sink::{parse_rows, ResumeMode, RunSink};
pub use trace::{RunManifest, TraceBuilder, TraceMeta, Tracer};
