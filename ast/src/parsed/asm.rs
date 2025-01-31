use std::{
    fmt::{Display, Formatter, Result},
    iter::{empty, once, repeat},
    str::FromStr,
};

use itertools::Itertools;
use powdr_number::BigUint;

use derive_more::From;

use crate::SourceRef;

use super::{Expression, PilStatement, TypedExpression};

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct ASMProgram {
    pub main: ASMModule,
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct ASMModule {
    pub statements: Vec<ModuleStatement>,
}

impl ASMModule {
    pub fn symbol_definitions(&self) -> impl Iterator<Item = &SymbolDefinition> {
        self.statements.iter().map(|s| match s {
            ModuleStatement::SymbolDefinition(d) => d,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, From)]
pub enum ModuleStatement {
    SymbolDefinition(SymbolDefinition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDefinition {
    pub name: String,
    pub value: SymbolValue,
}

#[derive(Debug, Clone, PartialEq, Eq, From)]
pub enum SymbolValue {
    /// A machine definition
    Machine(Machine),
    /// An import of a symbol from another module
    Import(Import),
    /// A module definition
    Module(Module),
    /// A generic symbol / function.
    Expression(TypedExpression),
}

impl SymbolValue {
    pub fn as_ref(&self) -> SymbolValueRef {
        match self {
            SymbolValue::Machine(machine) => SymbolValueRef::Machine(machine),
            SymbolValue::Import(i) => SymbolValueRef::Import(i),
            SymbolValue::Module(m) => SymbolValueRef::Module(m.as_ref()),
            SymbolValue::Expression(e) => SymbolValueRef::Expression(e),
        }
    }
}

#[derive(Debug, PartialEq, Eq, From)]
pub enum SymbolValueRef<'a> {
    /// A machine definition
    Machine(&'a Machine),
    /// An import of a symbol from another module
    Import(&'a Import),
    /// A module definition
    Module(ModuleRef<'a>),
    /// A generic symbol / function.
    Expression(&'a TypedExpression),
}

#[derive(Debug, Clone, PartialEq, Eq, From)]
pub enum Module {
    External(String),
    Local(ASMModule),
}

impl Module {
    fn as_ref(&self) -> ModuleRef {
        match self {
            Module::External(n) => ModuleRef::External(n),
            Module::Local(m) => ModuleRef::Local(m),
        }
    }
}

#[derive(Debug, PartialEq, Eq, From)]
pub enum ModuleRef<'a> {
    External(&'a str),
    Local(&'a ASMModule),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Import {
    /// the path imported in the source
    pub path: SymbolPath,
}

/// A symbol path is a sequence of strings separated by ``::`.
/// It can contain the special word `super`, which goes up a level.
/// If it does not start with `::`, it is relative.
#[derive(Default, Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct SymbolPath {
    /// The parts between each `::`.
    parts: Vec<Part>,
}

impl SymbolPath {
    pub fn from_identifier(name: String) -> Self {
        Self {
            parts: vec![Part::Named(name)],
        }
    }

    pub fn from_parts<P: IntoIterator<Item = Part>>(parts: P) -> Self {
        Self {
            parts: parts.into_iter().collect(),
        }
    }

    pub fn join<P: Into<Self>>(mut self, other: P) -> Self {
        self.parts.extend(other.into().parts);
        self
    }

    /// Formats the path and uses `.` as separator if
    /// there are at most two components.
    pub fn to_dotted_string(&self) -> String {
        let separator = if self.parts.len() <= 2 { "." } else { "::" };
        self.parts.iter().format(separator).to_string()
    }

    pub fn try_to_identifier(&self) -> Option<&String> {
        match &self.parts[..] {
            [Part::Named(name)] => Some(name),
            _ => None,
        }
    }

    pub fn try_last_part_mut(&mut self) -> Option<&mut String> {
        self.parts.last_mut().and_then(|p| match p {
            Part::Super => None,
            Part::Named(n) => Some(n),
        })
    }

    pub fn try_last_part(&self) -> Option<&String> {
        self.parts.last().and_then(|p| match p {
            Part::Super => None,
            Part::Named(n) => Some(n),
        })
    }

    /// Returns the last part of the path. Panics if it is "super" or if the path is empty.
    pub fn name(&self) -> &String {
        self.try_last_part().unwrap()
    }

    pub fn parts(&self) -> impl DoubleEndedIterator + ExactSizeIterator<Item = &Part> {
        self.parts.iter()
    }
}

impl FromStr for SymbolPath {
    type Err = String;

    /// Parses a symbol path both in the "a.b" and the "a::b" notation.
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        let (dots, double_colons) = (s.matches('.').count(), s.matches("::").count());
        if dots != 0 && double_colons != 0 {
            Err(format!("Path mixes \"::\" and \".\" separators: {s}"))?
        }
        let parts = s
            .split(if double_colons > 0 { "::" } else { "." })
            .map(|s| {
                if s == "super" {
                    Part::Super
                } else {
                    Part::Named(s.to_string())
                }
            })
            .collect();
        Ok(Self { parts })
    }
}

impl From<AbsoluteSymbolPath> for SymbolPath {
    fn from(value: AbsoluteSymbolPath) -> Self {
        Self {
            parts: once(String::new())
                .chain(value.parts)
                .map(Part::Named)
                .collect(),
        }
    }
}

impl Display for SymbolPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.parts.iter().format("::"))
    }
}

/// An absolute symbol path is a resolved SymbolPath,
/// which means it has to start with `::` and it cannot contain
/// the word `super`.
#[derive(Default, Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct AbsoluteSymbolPath {
    /// Contains the parts after the initial `::`.
    parts: Vec<String>,
}

/// Parses a path like `::path::to::symbol`.
/// Panics if the path does not start with '::'.
pub fn parse_absolute_path(s: &str) -> AbsoluteSymbolPath {
    match s.strip_prefix("::") {
        Some("") => AbsoluteSymbolPath::default(),
        Some(s) => s
            .split("::")
            .fold(AbsoluteSymbolPath::default(), |path, part| {
                path.with_part(part)
            }),
        None => panic!("Absolute symbol path does not start with '::': {s}"),
    }
}

impl AbsoluteSymbolPath {
    /// Removes and returns the last path component (unless empty).
    pub fn pop(&mut self) -> Option<String> {
        self.parts.pop()
    }

    /// Returns the path one level higher.
    pub fn parent(mut self) -> AbsoluteSymbolPath {
        self.pop().unwrap();
        self
    }

    pub fn len(&self) -> usize {
        self.parts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn parts(&self) -> impl DoubleEndedIterator + ExactSizeIterator<Item = &str> {
        self.parts.iter().map(|p| p.as_str())
    }

    /// Returns an iterator over all paths (not parts!) from self to the root.
    pub fn iter_to_root(&self) -> impl Iterator<Item = AbsoluteSymbolPath> + '_ {
        (0..=self.parts.len()).rev().map(|i| AbsoluteSymbolPath {
            parts: self.parts[..i].to_vec(),
        })
    }

    /// Appends a part to the end of the path.
    pub fn push(&mut self, part: String) {
        self.parts.push(part);
    }

    /// Returns the relative path from base to self.
    /// In other words, base.join(self.relative_to(base)) == self.
    pub fn relative_to(&self, base: &AbsoluteSymbolPath) -> SymbolPath {
        let common_prefix_len = self.common_prefix(base).parts.len();
        // Start with max(0, base.parts.len() - common_root.parts.len())
        // repetitions of "super".
        let parts = repeat(Part::Super)
            .take(base.parts.len().saturating_sub(common_prefix_len))
            // append the parts of self after the common root.
            .chain(
                self.parts
                    .iter()
                    .skip(common_prefix_len)
                    .cloned()
                    .map(Part::Named),
            )
            .collect();
        SymbolPath { parts }
    }

    /// Returns the common prefix of two paths.
    pub fn common_prefix(&self, other: &AbsoluteSymbolPath) -> AbsoluteSymbolPath {
        let parts = self
            .parts
            .iter()
            .zip(other.parts.iter())
            .map_while(|(a, b)| if a == b { Some(a.clone()) } else { None })
            .collect();

        AbsoluteSymbolPath { parts }
    }

    /// Resolves a relative path in the context of this absolute path.
    pub fn join<P: Into<SymbolPath>>(mut self, other: P) -> Self {
        for part in other.into().parts {
            match part {
                Part::Super => {
                    self.pop().unwrap();
                }
                Part::Named(name) => {
                    if name.is_empty() {
                        self.parts.clear();
                    } else {
                        self.parts.push(name);
                    }
                }
            }
        }
        self
    }

    /// Appends a part to the end of the path and returns a new copy.
    pub fn with_part(&self, part: &str) -> Self {
        assert!(!part.is_empty());
        let mut parts = self.parts.clone();
        parts.push(part.to_string());
        Self { parts }
    }

    /// Formats the path without leading `::` and uses `.` as separator if
    /// there are at most two components.
    pub fn to_dotted_string(&self) -> String {
        let separator = if self.parts.len() <= 2 { "." } else { "::" };
        self.parts.join(separator)
    }
}

impl Display for AbsoluteSymbolPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "::{}", self.parts.iter().format("::"))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum Part {
    Super,
    Named(String),
}

impl TryInto<String> for Part {
    type Error = ();

    fn try_into(self) -> std::result::Result<String, Self::Error> {
        if let Part::Named(name) = self {
            Ok(name)
        } else {
            Err(())
        }
    }
}

impl Display for Part {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Part::Super => write!(f, "super"),
            Part::Named(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Machine {
    pub arguments: MachineArguments,
    pub statements: Vec<MachineStatement>,
}

impl Machine {
    /// Returns a vector of all local variables / names defined in the machine.
    pub fn local_names(&self) -> Box<dyn Iterator<Item = &String> + '_> {
        Box::new(self.statements.iter().flat_map(|s| match s {
            MachineStatement::RegisterDeclaration(_, name, _) => Box::new(once(name)),
            MachineStatement::Pil(_, statement) => statement.symbol_definition_names(),
            MachineStatement::Degree(_, _)
            | MachineStatement::Submachine(_, _, _)
            | MachineStatement::InstructionDeclaration(_, _, _)
            | MachineStatement::LinkDeclaration(_, _)
            | MachineStatement::FunctionDeclaration(_, _, _, _)
            | MachineStatement::OperationDeclaration(_, _, _, _) => Box::new(empty()),
        }))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default, Clone)]
pub struct MachineArguments {
    pub latch: Option<String>,
    pub operation_id: Option<String>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Default)]
pub struct Params {
    pub inputs: Vec<Param>,
    pub outputs: Vec<Param>,
}

impl Params {
    pub fn inputs_and_outputs(&self) -> impl Iterator<Item = &Param> {
        self.inputs.iter().chain(self.outputs.iter())
    }

    pub fn inputs_and_outputs_mut(&mut self) -> impl Iterator<Item = &mut Param> {
        self.inputs.iter_mut().chain(self.outputs.iter_mut())
    }
}

impl Params {
    pub fn new(inputs: Vec<Param>, outputs: Vec<Param>) -> Self {
        Self { inputs, outputs }
    }

    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty() && self.outputs.is_empty()
    }

    pub fn prepend_space_if_non_empty(&self) -> String {
        let mut params_str = self.to_string();
        if !self.is_empty() {
            params_str = format!(" {params_str}");
        }
        params_str
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
/// the operation id necessary to call this function from the outside
pub struct OperationId {
    pub id: Option<BigUint>,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct Instruction {
    pub params: Params,
    pub body: InstructionBody,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum MachineStatement {
    Degree(SourceRef, BigUint),
    Pil(SourceRef, PilStatement),
    Submachine(SourceRef, SymbolPath, String),
    RegisterDeclaration(SourceRef, String, Option<RegisterFlag>),
    InstructionDeclaration(SourceRef, String, Instruction),
    LinkDeclaration(SourceRef, LinkDeclaration),
    FunctionDeclaration(SourceRef, String, Params, Vec<FunctionStatement>),
    OperationDeclaration(SourceRef, String, OperationId, Params),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct LinkDeclaration {
    pub flag: Expression,
    pub to: CallableRef,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct CallableRef {
    pub instance: String,
    pub callable: String,
    pub params: Params,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum InstructionBody {
    Local(Vec<PilStatement>),
    CallableRef(CallableRef),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssignmentRegister {
    Register(String),
    Wildcard,
}

impl AssignmentRegister {
    pub fn unwrap(self) -> String {
        match self {
            AssignmentRegister::Register(r) => r,
            AssignmentRegister::Wildcard => panic!("cannot unwrap wildcard"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum FunctionStatement {
    Assignment(
        SourceRef,
        Vec<String>,
        Option<Vec<AssignmentRegister>>,
        Box<Expression>,
    ),
    Instruction(SourceRef, String, Vec<Expression>),
    Label(SourceRef, String),
    DebugDirective(SourceRef, DebugDirective),
    Return(SourceRef, Vec<Expression>),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum DebugDirective {
    File(usize, String, String),
    Loc(usize, usize, usize),
    OriginalInstruction(String),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum RegisterFlag {
    IsPC,
    IsAssignment,
    IsReadOnly,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Param {
    pub name: String,
    pub index: Option<BigUint>,
    pub ty: Option<String>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn common_prefix() {
        assert_eq!(
            parse_absolute_path("::a::b").common_prefix(&parse_absolute_path("::a::c")),
            parse_absolute_path("::a")
        );
        assert_eq!(
            parse_absolute_path("::a::b").common_prefix(&parse_absolute_path("::a")),
            parse_absolute_path("::a")
        );
        assert_eq!(
            parse_absolute_path("::a").common_prefix(&parse_absolute_path("::a::c")),
            parse_absolute_path("::a")
        );
        assert_eq!(
            parse_absolute_path("::x").common_prefix(&parse_absolute_path("::y::t")),
            parse_absolute_path("::")
        );
        assert_eq!(
            parse_absolute_path("::x::r::v").common_prefix(&parse_absolute_path("::x::r::t")),
            parse_absolute_path("::x::r")
        );
    }

    #[test]
    fn relative_to() {
        assert_eq!(
            parse_absolute_path("::a::b")
                .relative_to(&parse_absolute_path("::a::c"))
                .to_string(),
            "super::b".to_string()
        );
        assert_eq!(
            parse_absolute_path("::a::b")
                .relative_to(&parse_absolute_path("::a"))
                .to_string(),
            "b".to_string()
        );
        assert_eq!(
            parse_absolute_path("::x")
                .relative_to(&parse_absolute_path("::y::t"))
                .to_string(),
            "super::super::x".to_string()
        );
        assert_eq!(
            parse_absolute_path("::x::r::v")
                .relative_to(&parse_absolute_path("::x::r"))
                .to_string(),
            "v".to_string()
        );
        assert_eq!(
            parse_absolute_path("::x")
                .relative_to(&parse_absolute_path("::x::t::k"))
                .to_string(),
            "super::super".to_string()
        );
        assert_eq!(
            parse_absolute_path("::x")
                .relative_to(&parse_absolute_path("::x"))
                .to_string(),
            "".to_string()
        );
    }

    #[test]
    fn relative_to_join() {
        let v = parse_absolute_path("::x::r::v");
        let base = parse_absolute_path("::x::t");
        let rel = v.relative_to(&base);
        assert_eq!(base.join(rel), v);
    }
}
