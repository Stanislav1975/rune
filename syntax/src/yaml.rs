use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Formatter, Display},
    str::FromStr,
};
use regex::Regex;
use once_cell::sync::Lazy;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use codespan_reporting::files::{Files, SimpleFile};
use codespan::Span;
use petgraph::graph::NodeIndex;
use crate::{
    Diagnostics,
    analysis::{Builtins, HirIds},
    hir::{self, HirId, Rune},
    ast::{ArgumentValue, Literal},
};

type FileId =
    <SimpleFile<&'static str, &'static str> as Files<'static>>::FileId;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub image: Path,
    pub pipeline: HashMap<String, Stage>,
}

impl Document {
    pub fn parse(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }
}

impl FromStr for Document {
    type Err = serde_yaml::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> { Document::parse(s) }
}

/// A specification for finding a dependency.
///
/// The full syntax is `base@version#sub_path` where
///
/// - `base` is a URL or the name of a repository on GitHub (e.g. `hotg-ai/rune`
///   or `https://github.com/hotg-ai/rune`)
/// - `version` is an optional field specifying the version (e.g. as a git tag)
/// - `sub_path` is an optional field which is useful when pointing to
///   repositories with multiple relevant items because it lets you specify
///   which directory the specified item is in.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Path {
    pub base: String,
    pub sub_path: Option<String>,
    pub version: Option<String>,
}

impl Path {
    pub fn new(
        base: impl Into<String>,
        sub_path: impl Into<Option<String>>,
        version: impl Into<Option<String>>,
    ) -> Self {
        Path {
            base: base.into(),
            sub_path: sub_path.into(),
            version: version.into(),
        }
    }
}

impl<'a> From<&'a Path> for crate::ast::Path {
    fn from(p: &'a Path) -> crate::ast::Path {
        let Path {
            base,
            sub_path,
            version,
        } = p;
        crate::ast::Path::new(
            base.clone(),
            sub_path.clone(),
            version.clone(),
            Span::new(0, 0),
        )
    }
}

impl Display for Path {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Path {
            base,
            sub_path,
            version,
        } = self;

        write!(f, "{}", base)?;
        if let Some(sub) = sub_path {
            write!(f, "#{}", sub)?;
        }
        if let Some(version) = version {
            write!(f, "@{}", version)?;
        }

        Ok(())
    }
}

impl FromStr for Path {
    type Err = PathParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        static PATTERN: Lazy<Regex> = Lazy::new(|| {
            Regex::new(
                r"(?x)
        (?P<base>[\w\d:/_.-]+)
        (?:@(?P<version>[\w\d./-]+))?
        (?:\#(?P<sub_path>[\w\d._/-]+))?
        ",
            )
            .unwrap()
        });

        let captures = PATTERN.captures(s).ok_or(PathParseError)?;

        let base = captures["base"].to_string();
        let version = captures.name("version").map(|m| m.as_str().to_string());
        let sub_path =
            captures.name("sub_path").map(|m| m.as_str().to_string());

        Ok(Path {
            base,
            version,
            sub_path,
        })
    }
}

impl Serialize for Path {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Path {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Cow::<'de, str>::deserialize(deserializer)?;

        s.parse().map_err(D::Error::custom)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct PathParseError;

impl Display for PathParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Unable to parse the path")
    }
}

impl std::error::Error for PathParseError {}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum Stage {
    Model {
        model: String,
        #[serde(default)]
        inputs: Vec<String>,
        #[serde(default)]
        outputs: Vec<Type>,
    },
    ProcBlock {
        #[serde(rename = "proc-block")]
        proc_block: Path,
        #[serde(default)]
        inputs: Vec<String>,
        #[serde(default)]
        outputs: Vec<Type>,
        #[serde(default)]
        args: HashMap<String, Value>,
    },
    Capability {
        capability: String,
        #[serde(default)]
        outputs: Vec<Type>,
        #[serde(default)]
        args: HashMap<String, Value>,
    },
    Out {
        out: String,
        #[serde(default)]
        inputs: Vec<String>,
        #[serde(default)]
        args: HashMap<String, Value>,
    },
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct Type {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub dimensions: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "kebab-case", untagged)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
}

impl From<f64> for Value {
    fn from(f: f64) -> Value { Value::Float(f) }
}

impl From<i64> for Value {
    fn from(i: i64) -> Value { Value::Int(i) }
}

impl From<String> for Value {
    fn from(s: String) -> Value { Value::String(s) }
}

impl<'a> From<&'a str> for Value {
    fn from(s: &'a str) -> Value { Value::String(s.to_string()) }
}

impl From<Vec<Value>> for Value {
    fn from(list: Vec<Value>) -> Value { Value::List(list) }
}

impl From<Value> for ArgumentValue {
    fn from(v: Value) -> ArgumentValue {
        match v {
            Value::Int(i) => {
                ArgumentValue::Literal(Literal::new(i, Span::new(0, 0)))
            },
            Value::Float(f) => {
                ArgumentValue::Literal(Literal::new(f, Span::new(0, 0)))
            },
            Value::String(s) => {
                ArgumentValue::Literal(Literal::new(s, Span::new(0, 0)))
            },
            Value::List(list) => {
                let mut items = Vec::new();
                for item in list {
                    if let Value::String(s) = item {
                        items.push(s.clone());
                    } else {
                        unimplemented!();
                    }
                }

                ArgumentValue::List(items)
            },
        }
    }
}

pub fn analyse(doc: &Document) -> (Rune, Diagnostics<FileId>) {
    let mut ctx = Context::default();

    ctx.register_names(&doc.pipeline);
    ctx.register_stages(&doc.pipeline);
    ctx.construct_pipeline(&doc.pipeline);

    let Context { rune, diags, .. } = ctx;

    (rune, diags)
}

#[derive(Debug)]
struct Context {
    diags: Diagnostics<FileId>,
    rune: Rune,
    ids: HirIds,
    builtins: Builtins,
    stages: HashMap<HirId, NodeIndex>,
    input_types: HashMap<NodeIndex, HirId>,
    output_types: HashMap<NodeIndex, HirId>,
}

impl Context {
    fn register_names(&mut self, pipeline: &HashMap<String, Stage>) {
        for (name, _step) in pipeline {
            let id = self.ids.next();
            self.rune.names.register(name, id);
        }
    }

    fn register_stages(&mut self, pipeline: &HashMap<String, Stage>) {
        for (name, stage) in pipeline {
            let id = self.rune.names[name.as_str()];

            let node = match stage {
                Stage::Model { model, .. } => hir::Stage::Model(hir::Model {
                    model_file: model.into(),
                }),
                Stage::ProcBlock {
                    proc_block, args, ..
                } => hir::Stage::ProcBlock(hir::ProcBlock {
                    path: proc_block.into(),
                    parameters: to_parameters(args),
                }),
                Stage::Capability {
                    capability, args, ..
                } => hir::Stage::Source(hir::Source {
                    kind: capability.as_str().into(),
                    parameters: to_parameters(args),
                }),
                Stage::Out { out, .. } => hir::Stage::Sink(hir::Sink {
                    kind: out.as_str().into(),
                }),
            };

            let node_index = self.rune.graph.add_node(node);
            self.rune.add_hir_id_and_node_index(id, node_index);
        }
    }

    fn get_type(&mut self, ty: &Type) -> HirId { todo!() }

    fn construct_pipeline(&mut self, _steps: &HashMap<String, Stage>) {
        todo!()
    }
}

fn to_parameters(
    yaml: &HashMap<String, Value>,
) -> HashMap<String, ArgumentValue> {
    let mut map = HashMap::new();

    for (key, value) in yaml {
        map.insert(key.clone(), value.clone().into());
    }

    map
}

impl Default for Context {
    fn default() -> Context {
        let mut ids = HirIds::new();
        let builtins = Builtins::new(&mut ids);

        Context {
            ids,
            builtins,
            rune: Rune::default(),
            diags: Diagnostics::default(),
            stages: HashMap::default(),
            input_types: HashMap::default(),
            output_types: HashMap::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! map {
    // map-like
    ($($k:expr => $v:expr),* $(,)?) => {
        std::iter::Iterator::collect(std::array::IntoIter::new([$(($k, $v),)*]))
    };
    // set-like
    ($($v:expr),* $(,)?) => {
        std::iter::Iterator::collect(std::array::IntoIter::new([$($v,)*]))
    };
}

    #[test]
    fn parse_yaml_pipeline() {
        let src = r#"
image: "runicos/base"

pipeline:
  audio:
    capability: SOUND
    outputs:
    - type: i16
      dimensions: [16000]
    args:
      hz: 16000

  fft:
    proc-block: "hotg-ai/rune#proc_blocks/fft"
    inputs:
    - audio
    outputs:
    - type: i8
      dimensions: [1960]

  model:
    model: "./model.tflite"
    inputs:
    - fft
    outputs:
    - type: i8
      dimensions: [6]

  label:
    proc-block: "hotg-ai/rune#proc_blocks/ohv_label"
    inputs:
    - model
    outputs:
    - type: utf8
    args:
      labels: ["silence", "unknown", "up", "down", "left", "right"]

  output:
    out: SERIAL
    inputs:
    - label
        "#;
        let should_be = Document {
            image: Path::new("runicos/base", None, None),
            pipeline: map! {
                "audio".into() => Stage::Capability {
                    capability: String::from("SOUND"),
                    outputs: vec![Type {
                        ty: String::from("i16"),
                        dimensions: vec![16000],
                    }],
                    args: map! { "hz".to_string() => Value::Int(16000) },
                },
                "output".into() => Stage::Out {
                    out: String::from("SERIAL"),
                    args: HashMap::new(),
                    inputs: vec![String::from("label")],
                },
                "label".into() => Stage::ProcBlock {
                    proc_block: "hotg-ai/rune#proc_blocks/ohv_label".parse().unwrap(),
                    inputs: vec![String::from("model")],
                    outputs: vec![Type { ty: String::from("utf8"), dimensions: Vec::new() }],
                    args: map! {
                        String::from("labels") => Value::from(vec![
                            Value::from("silence"),
                            Value::from("unknown"),
                            Value::from("up"),
                            Value::from("down"),
                            Value::from("left"),
                            Value::from("right"),
                        ]),
                    },
                },
                "fft".into() => Stage::ProcBlock {
                    proc_block: "hotg-ai/rune#proc_blocks/fft".parse().unwrap(),
                    inputs: vec![String::from("audio")],
                    outputs: vec![Type { ty: String::from("i8"), dimensions: vec![1960] }],
                    args: HashMap::new(),
                },
                "model".into() => Stage::Model {
                    model: String::from("./model.tflite"),
                    inputs: vec![String::from("fft")],
                    outputs: vec![Type { ty: String::from("i8"), dimensions: vec![6] }],
                },
            },
        };

        let got: Document = serde_yaml::from_str(src).unwrap();

        assert_eq!(got, should_be);
    }

    #[test]
    fn parse_audio_block() {
        let src = r#"
              capability: SOUND
              outputs:
              - type: i16
                dimensions: [16000]
              args:
                hz: 16000
        "#;
        let should_be = Stage::Capability {
            capability: String::from("SOUND"),
            outputs: vec![Type {
                ty: String::from("i16"),
                dimensions: vec![16000],
            }],
            args: map! { "hz".to_string() => Value::Int(16000) },
        };

        let got: Stage = serde_yaml::from_str(src).unwrap();

        assert_eq!(got, should_be);
    }

    #[test]
    fn parse_values() {
        let inputs = vec![
            ("42", Value::Int(42)),
            ("3.14", Value::Float(3.14)),
            ("\"42\"", Value::String(String::from("42"))),
            (
                "[1, 2.0, \"asdf\"]",
                Value::List(vec![
                    Value::Int(1),
                    Value::Float(2.0),
                    Value::String(String::from("asdf")),
                ]),
            ),
        ];

        for (src, should_be) in inputs {
            let got: Value = serde_yaml::from_str(src).unwrap();
            assert_eq!(got, should_be);
        }
    }

    #[test]
    fn parse_paths() {
        let inputs = vec![
            ("asdf", Path::new("asdf", None, None)),
            ("runicos/base", Path::new("runicos/base", None, None)),
            (
                "runicos/base@0.1.2",
                Path::new("runicos/base", None, "0.1.2".to_string()),
            ),
            (
                "runicos/base@latest",
                Path::new("runicos/base", None, "latest".to_string()),
            ),
            (
                "https://github.com/hotg-ai/rune",
                Path::new("https://github.com/hotg-ai/rune", None, None),
            ),
            (
                "https://github.com/hotg-ai/rune@2",
                Path::new(
                    "https://github.com/hotg-ai/rune",
                    None,
                    "2".to_string(),
                ),
            ),
            (
                "hotg-ai/rune@v1.2#proc_blocks/normalize",
                Path::new(
                    "hotg-ai/rune",
                    "proc_blocks/normalize".to_string(),
                    "v1.2".to_string(),
                ),
            ),
        ];

        for (src, should_be) in inputs {
            let got: Path = src.parse().unwrap();
            assert_eq!(got, should_be);
        }
    }

    macro_rules! map {
        ($($key:ident : $value:expr),* $(,)?) => {
            vec![
                $(
                    (stringify!($key).to_string(), $value)
                ),*
            ]
            .into_iter()
            .collect()
        };
    }

    macro_rules! ty {
        ($type:ident [$($dim:expr),*]) => {
            Type {
                ty: String::from(stringify!($type)),
                dimensions: vec![ $($dim),*],
            }
        };
        ($type:ident) => {
            Type {
                ty: String::from(stringify!($type)),
                dimensions: vec![],
            }
        }
    }

    fn dummy_document() -> Document {
        Document {
            image: Path::new("runicos/base".to_string(), None, None),
            pipeline: map! {
                audio: Stage::Capability {
                    capability: String::from("SOUND"),
                    outputs: vec![
                        ty!(i16[16000]),
                    ],
                    args: map! {
                        hz: Value::from(16000),
                    },
                },
                fft: Stage::ProcBlock {
                    proc_block: "hotg-ai/rune#proc_blocks/fft".parse().unwrap(),
                    inputs: vec![String::from("audio")],
                    outputs: vec![
                        ty!(i8[1960]),
                    ],
                    args: HashMap::new(),
                },
                model: Stage::Model {
                    model: String::from("./model.tflite"),
                    inputs: vec![String::from("fft")],
                    outputs: vec![
                        ty!(i8[6]),
                    ],
                },
                label: Stage::ProcBlock {
                    proc_block: "hotg-ai/rune#proc_blocks/ohv_label".parse().unwrap(),
                    inputs: vec![String::from("model")],
                    outputs: vec![
                        ty!(utf8),
                    ],
                    args: map! {
                        labels: Value::List(vec![
                            Value::from("silence"),
                            Value::from("unknown"),
                            Value::from("up"),
                        ]),
                    },
                },
                output: Stage::Out {
                    out: String::from("SERIAL"),
                    inputs: vec![String::from("label")],
                    args: HashMap::default(),
                }
            },
        }
    }

    #[test]
    fn register_all_stage_names() {
        let doc = dummy_document();
        let mut ctx = Context::default();

        ctx.register_names(&doc.pipeline);

        let expected = vec!["audio", "fft", "model", "label", "output"];
        let got = &ctx.rune.names;

        for name in expected {
            assert!(got.get_id(name).is_some(), "{}", name);
        }
    }

    #[test]
    fn register_all_stages() {
        let doc = dummy_document();
        let mut ctx = Context::default();
        let stages = vec!["audio", "fft", "model", "label", "output"];
        ctx.register_names(&doc.pipeline);

        ctx.register_stages(&doc.pipeline);

        for ty in stages {
            let id = ctx.rune.names[ty];
            let node_index = ctx.rune.hir_id_to_node_index[&id];
            assert!(ctx.rune.graph.node_weight(node_index).is_some());
        }
    }
}
