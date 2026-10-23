//! KDL 2.0 abstract syntax tree (T0501).
//!
//! The AST preserves **every** node, entry and value, including duplicates
//! and unknown fields. Schema rejection (T0511) is performed by the
//! consumer (`elang-epf`, `elang-eld`, `elang-integrity`) over this tree;
//! the parser never silently drops anything.
//!
//! All AST types are `Clone` and own their children via `Box<[T]>` for
//! deterministic memory layout. Source spans are stored alongside each
//! node so diagnostics can highlight the offending construct.

use crate::span::SourceSpan;
use crate::value::KdlValue;

/// One positional argument or named property of a node.
///
/// KDL distinguishes the two by presence of `key`: an entry without a key
/// is a positional argument; an entry with a key is a property whose value
/// is on the right-hand side of `=`.
#[derive(Debug, Clone, PartialEq)]
pub enum KdlEntry {
    /// Positional argument.
    Arg(KdlValue),
    /// Named property (`key=value`).
    Prop {
        /// Source span of the property name.
        key_span: SourceSpan,
        /// Property name (verbatim identifier — no type annotation in KDL 2.0
        /// for property keys).
        key: Box<str>,
        /// Value on the right-hand side of `=`.
        value: KdlValue,
    },
}

impl KdlEntry {
    /// Source span covering the entire entry (including `=` for properties).
    pub fn span(&self) -> SourceSpan {
        match self {
            KdlEntry::Arg(v) => v.span(),
            KdlEntry::Prop {
                key_span, value, ..
            } => SourceSpan {
                start: key_span.start,
                end: value.span().end,
            },
        }
    }

    /// True if this entry is a positional argument.
    pub fn is_arg(&self) -> bool {
        matches!(self, KdlEntry::Arg(_))
    }

    /// True if this entry is a property (`key=value`).
    pub fn is_prop(&self) -> bool {
        matches!(self, KdlEntry::Prop { .. })
    }
}

/// Children block: a list of nodes inside `{ … }`.
///
/// `KdlChildren` is its own type (not just `Vec<KdlNode>`) so the
/// per-block node budget (T0507) can be enforced locally: each child block
/// is bounded by `MAX_NODES_PER_RESOURCE` as part of the enclosing
/// resource's total, not as an independent total.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KdlChildren {
    /// Ordered list of children.
    nodes: Box<[KdlNode]>,
}

impl KdlChildren {
    /// Construct from a boxed slice of nodes.
    pub fn new(nodes: Box<[KdlNode]>) -> KdlChildren {
        KdlChildren { nodes }
    }

    /// Borrow the children as a slice.
    pub fn as_slice(&self) -> &[KdlNode] {
        &self.nodes
    }

    /// Number of children.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True when there are no children.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Iterate over the children.
    pub fn iter(&self) -> core::slice::Iter<'_, KdlNode> {
        self.nodes.iter()
    }
}

impl core::ops::Deref for KdlChildren {
    type Target = [KdlNode];
    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

/// A single KDL node.
///
/// `name` is the verbatim identifier after the optional type annotation.
/// `ty` is the verbatim identifier inside `(…)` when present; absent
/// otherwise. `entries` are positional then named in source order.
/// `children` is present iff the node had a `{ … }` block in source.
#[derive(Debug, Clone, PartialEq)]
pub struct KdlNode {
    /// Source span of the full node including any children block.
    pub span: SourceSpan,
    /// Optional type annotation `(type)`.
    pub ty: Option<(SourceSpan, Box<str>)>,
    /// Node name.
    pub name: (SourceSpan, Box<str>),
    /// Arguments and properties, in source order.
    pub entries: Box<[KdlEntry]>,
    /// Children block, if present.
    pub children: Option<KdlChildren>,
}

impl KdlNode {
    /// Source span of the full node.
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    /// Node name as a string slice.
    pub fn name(&self) -> &str {
        &self.name.1
    }

    /// Optional type annotation.
    pub fn ty(&self) -> Option<&str> {
        self.ty.as_ref().map(|(_, s)| s.as_ref())
    }

    /// Borrow the entries (arguments and properties).
    pub fn entries(&self) -> &[KdlEntry] {
        &self.entries
    }

    /// Borrow the children block, if any.
    pub fn children(&self) -> Option<&KdlChildren> {
        self.children.as_ref()
    }

    /// Positional arguments, in source order.
    pub fn args(&self) -> impl Iterator<Item = &KdlValue> {
        self.entries.iter().filter_map(|e| {
            if let KdlEntry::Arg(v) = e {
                Some(v)
            } else {
                None
            }
        })
    }

    /// Properties, in source order.
    pub fn props(&self) -> impl Iterator<Item = (&str, &KdlValue)> {
        self.entries.iter().filter_map(|e| {
            if let KdlEntry::Prop { key, value, .. } = e {
                Some((key.as_ref(), value))
            } else {
                None
            }
        })
    }
}

/// The optional `/kdl-version` pragma that precedes the document.
///
/// KDL 2.0 requires the pragma to appear before any node. The parser
/// enforces its presence (T0501) and rejects documents that omit it or
/// that name a version other than `2.0`.
#[derive(Debug, Clone, PartialEq)]
pub struct KdlVersionPragma {
    /// Source span of the full pragma.
    pub span: SourceSpan,
    /// Parsed version, currently always `"2.0"`.
    pub version: Box<str>,
}

/// A parsed KDL document.
#[derive(Debug, Clone, PartialEq)]
pub struct KdlDocument {
    /// Span of the full document.
    pub span: SourceSpan,
    /// The `/kdl-version "2.0"` pragma, if present (mandatory for KDL 2.0).
    pub version: Option<KdlVersionPragma>,
    /// Document nodes, in source order.
    pub nodes: Box<[KdlNode]>,
}

impl KdlDocument {
    /// Source span.
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    /// Document nodes, in source order.
    pub fn nodes(&self) -> &[KdlNode] {
        &self.nodes
    }

    /// The version pragma, if the parser required and found one.
    pub fn version(&self) -> Option<&KdlVersionPragma> {
        self.version.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> (SourceSpan, Box<str>) {
        (SourceSpan::INVALID, s.into())
    }

    fn value_null() -> KdlValue {
        KdlValue::Null(SourceSpan::INVALID)
    }

    #[test]
    fn entry_arg_is_arg_not_prop() {
        let e = KdlEntry::Arg(value_null());
        assert!(e.is_arg());
        assert!(!e.is_prop());
    }

    #[test]
    fn entry_prop_has_key() {
        let e = KdlEntry::Prop {
            key_span: SourceSpan::INVALID,
            key: "x".into(),
            value: value_null(),
        };
        assert!(e.is_prop());
        assert!(!e.is_arg());
    }

    #[test]
    fn node_args_and_props_split() {
        let n = KdlNode {
            span: SourceSpan::INVALID,
            ty: None,
            name: name("foo"),
            entries: vec![
                KdlEntry::Arg(value_null()),
                KdlEntry::Prop {
                    key_span: SourceSpan::INVALID,
                    key: "k".into(),
                    value: value_null(),
                },
            ]
            .into_boxed_slice(),
            children: None,
        };
        assert_eq!(n.args().count(), 1);
        assert_eq!(n.props().count(), 1);
        assert_eq!(n.name(), "foo");
    }

    #[test]
    fn children_iterates_in_order() {
        let c = KdlChildren::new(
            vec![
                KdlNode {
                    span: SourceSpan::INVALID,
                    ty: None,
                    name: name("a"),
                    entries: Box::new([]),
                    children: None,
                },
                KdlNode {
                    span: SourceSpan::INVALID,
                    ty: None,
                    name: name("b"),
                    entries: Box::new([]),
                    children: None,
                },
            ]
            .into_boxed_slice(),
        );
        let names: Vec<&str> = c.iter().map(|n| n.name()).collect();
        assert_eq!(names, vec!["a", "b"]);
        assert_eq!(c.len(), 2);
    }
}
