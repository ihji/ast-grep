use crate::ast::convert_utils::{
  flatten_nested, map_transposed_or_default, transpose_with_ctx, with_ctx,
};
use crate::ast::go_nodes::anon_unions::Anon296608916358560338577356235414932652551 as CompositeLiteralType;
use crate::ast::go_nodes::anon_unions::Block_IfStatement;
use crate::ast::go_nodes::anon_unions::DefaultCase_ExpressionCase;
use crate::ast::go_nodes::anon_unions::DefaultCase_TypeCase;
use crate::ast::go_nodes::anon_unions::Expression_ForClause_RangeClause;
use crate::ast::go_nodes::anon_unions::Expression_LiteralValue;
use crate::ast::go_nodes::anon_unions::Expression_Type_VariadicArgument;
use crate::ast::go_nodes::anon_unions::KeyedElement_LiteralElement;
use crate::ast::go_nodes::anon_unions::NegatedType_QualifiedType_TypeIdentifier;
use crate::ast::go_nodes::anon_unions::NotEq_Mod_And_AndAnd_AndBitXor_Mul_Add_Sub_Div_Lt_LtLt_LtEq_EqEq_Gt_GtEq_GtGt_BitXor_Or_OrOr as BinaryOp;
use crate::ast::go_nodes::anon_unions::Not_And_Mul_Add_Sub_LtSub_BitXor as UnaryOp;
use crate::ast::go_nodes::anon_unions::ParameterDeclaration_VariadicParameterDeclaration as ParamDecl;
use crate::ast::go_nodes::anon_unions::Statement_FunctionDeclaration_ImportDeclaration_MethodDeclaration_PackageClause as TopLevel;
use crate::ast::go_nodes::*;
use crate::ast::source_info::SourceInfo;
use crate::il;
use crate::il::{StatementValue, ValueExtra};

use anyhow::anyhow;
use anyhow::Result;
use ast_grep_core::tree_sitter::StrDoc;
use ast_grep_core::AstGrep;
use ast_grep_language::SupportLang;
use type_sitter::HasChild;
use type_sitter::{HasChildren, Node};

use super::go_nodes::anon_unions::Comma_Identifier;
use super::go_nodes::anon_unions::Comma_Type;
use super::go_nodes::anon_unions::SimpleType_ParameterList;

use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TMP_VAR_COUNTER: AtomicUsize = AtomicUsize::new(0);

use std::sync::LazyLock;
use std::vec;

static BUILTIN_TYPES: LazyLock<HashMap<&'static str, il::Type>> = LazyLock::new(|| {
  [
    ("int", il::Type::Int),
    ("uint", il::Type::UInt),
    ("int8", il::Type::Byte),
    ("uint8", il::Type::UByte),
    ("int16", il::Type::Short),
    ("uint16", il::Type::UShort),
    ("int32", il::Type::Int),
    ("uint32", il::Type::UInt),
    ("int64", il::Type::Long),
    ("uint64", il::Type::ULong),
    ("float32", il::Type::Float),
    ("float64", il::Type::Double),
    ("string", il::Type::String),
    ("bool", il::Type::Bool),
    ("nil", il::Type::Null),
    (
      "map",
      il::Type::Named {
        name: "map".to_string(),
        generics: vec![il::Type::Any, il::Type::Any],
      },
    ),
    (
      "chan",
      il::Type::Named {
        name: "chan".to_string(),
        generics: vec![il::Type::Any],
      },
    ),
    ("any", il::Type::Any),
  ]
  .into_iter()
  .collect()
});

pub struct GoConverter<'src> {
  pub program: il::Program,
  pub source_info: SourceInfo<'src>,
}

impl<'src> GoConverter<'src> {
  pub fn new(path: PathBuf) -> Self {
    Self {
      program: il::Program::new(path),
      source_info: SourceInfo::new(),
    }
  }

  pub fn get_tmp_var(&mut self, t: Option<il::Type>) -> (il::Value, String) {
    let counter = TMP_VAR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = format!("%tmp_{}", counter);
    (
      il::Value {
        kind: il::ValueKind::Ident(name.clone()),
        extra: ValueExtra::new(t, None),
        tag: None,
      },
      name,
    )
  }

  fn collect_typed_params(values: Vec<il::Value>, context: &str) -> Vec<(il::Type, String)> {
    values
      .into_iter()
      .filter_map(|value| match value {
        il::Value {
          kind: il::ValueKind::Ident(name),
          extra: ValueExtra {
            type_declared: Some(t),
            ..
          },
          ..
        } => Some((t, name)),
        _ => {
          eprintln!("Expected typed Ident value in {context}, got: {:?}", value);
          None
        }
      })
      .collect()
  }

  fn convert_statement_results<E, I>(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    items: I,
    item_ctx: &str,
  ) -> Result<Vec<il::Statement>>
  where
    I: IntoIterator<Item = std::result::Result<Statement<'src>, E>>,
    E: Debug,
  {
    items
      .into_iter()
      .map(|item| {
        with_ctx(item, item_ctx).and_then(|statement| self.convert_statement(root, &statement))
      })
      .collect::<Result<Vec<Vec<il::Statement>>>>()
      .map(flatten_nested)
  }

  fn convert_optional_statement_list<E>(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    statement_list: Option<std::result::Result<StatementList<'src>, E>>,
    list_ctx: &str,
    item_ctx: &str,
  ) -> Result<Vec<il::Statement>>
  where
    E: Debug,
  {
    match transpose_with_ctx(statement_list, list_ctx)? {
      Some(statement_list) => self.convert_statement_results(
        root,
        statement_list.statements(&mut statement_list.walk()),
        item_ctx,
      ),
      None => Ok(vec![]),
    }
  }

  fn convert_expression_results<E, I>(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    items: I,
    item_ctx: &str,
  ) -> Result<Vec<(il::Value, Vec<il::Statement>)>>
  where
    I: IntoIterator<Item = std::result::Result<Expression<'src>, E>>,
    E: Debug,
  {
    items
      .into_iter()
      .map(|item| with_ctx(item, item_ctx).and_then(|expr| self.convert_expression(root, &expr)))
      .collect()
  }

  fn split_expression_results(
    values: Vec<(il::Value, Vec<il::Statement>)>,
  ) -> (Vec<il::Value>, Vec<il::Statement>) {
    let (values, stmts): (Vec<il::Value>, Vec<Vec<il::Statement>>) = values.into_iter().unzip();
    (values, flatten_nested(stmts))
  }

  fn default_true_condition() -> (il::Value, Vec<il::Statement>) {
    (
      il::Value {
        kind: il::ValueKind::IntLit(1),
        extra: ValueExtra::new(None, None),
        tag: None,
      },
      vec![],
    )
  }

  pub fn convert(&mut self, root: &'src AstGrep<StrDoc<SupportLang>>) -> () {
    eprintln!("Converting source file");
    let stmts = {
      let node = SourceFile::try_from_raw(root.root().get_inner_node()).unwrap();
      node
        .children(&mut node.walk())
        .map(|child| child.unwrap())
        .collect::<Vec<_>>()
    };
    match self.convert_toplevels(&root, &stmts) {
      Ok(stmts) => self.program.add_statements(stmts),
      Err(e) => eprintln!("Error converting top-level statements: {:?}", e),
    }
  }

  fn convert_parameter_declaration(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    param: &ParamDecl<'src>,
  ) -> Result<Vec<il::Value>> {
    match param {
      ParamDecl::ParameterDeclaration(pd) => {
        let names = pd
          .names(&mut pd.walk())
          .map(|name| Ok(name.utf8_text(root.source().as_bytes())?.to_string()))
          .collect::<Result<Vec<_>>>()?;
        let t = self.convert_type(root, &pd.r#type().unwrap());
        if names.is_empty() {
          Ok(vec![il::Value {
            kind: il::ValueKind::Ident("_".to_string()),
            extra: ValueExtra::new(Some(t.clone()), None),
            tag: None,
          }])
        } else {
          Ok(
            names
              .into_iter()
              .map(|name| {
                if BUILTIN_TYPES.contains_key(name.as_str()) {
                  il::Value {
                    kind: il::ValueKind::Ident("_".to_string()),
                    extra: ValueExtra::new(Some(BUILTIN_TYPES[name.as_str()].clone()), None),
                    tag: None,
                  }
                } else {
                  il::Value {
                    kind: il::ValueKind::Ident(name.clone()),
                    extra: ValueExtra::new(Some(t.clone()), None),
                    tag: None,
                  }
                }
              })
              .collect::<Vec<_>>(),
          )
        }
      }
      ParamDecl::VariadicParameterDeclaration(vd) => {
        let name = vd
          .name()
          .ok_or(anyhow!("Variadic parameter declaration has no name"))?
          .utf8_text(root.source().as_bytes())?
          .to_string();
        let t = self.convert_type(root, &vd.r#type().unwrap());
        Ok(vec![il::Value {
          kind: il::ValueKind::Ident(name),
          extra: ValueExtra::new(Some(il::Type::Array(Box::new(t.clone()), None)), None),
          tag: None,
        }])
      }
    }
  }

  fn convert_parameter_list(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    param_list: &ParameterList<'src>,
  ) -> Result<Vec<il::Value>> {
    param_list
      .children(&mut param_list.walk())
      .map(|plr| {
        with_ctx(plr, "Error converting parameter list")
          .and_then(|pl| self.convert_parameter_declaration(root, &pl))
      })
      .collect::<Result<Vec<Vec<il::Value>>>>()
      .map(flatten_nested)
  }

  fn convert_toplevels(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    tops: &Vec<TopLevel<'src>>,
  ) -> Result<Vec<il::Statement>> {
    // Convert each top-level declaration
    let results: Vec<Vec<il::Statement>> = tops
      .iter()
      .map(|top| -> Result<Vec<il::Statement>> {
        match top {
          TopLevel::Statement(st) => self.convert_statement(root, st),
          TopLevel::PackageClause(pc) => {
            let name = with_ctx(pc.package_identifier(), "Error converting package name")?
              .utf8_text(root.source().as_bytes())?
              .to_string();
            let tag = self.source_info.register(*pc.raw());
            Ok(vec![il::Statement::Define {
              kind: il::DefineKind::Package(name),
              scope_id: None,
              tag: Some(tag),
            }])
          }
          TopLevel::MethodDeclaration(md) => {
            let receiver_list = with_ctx(md.receiver(), "Error converting method receiver")?;
            let receiver_params = self.convert_parameter_list(root, &receiver_list)?;
            let method_param_list =
              with_ctx(md.parameters(), "Error converting method parameters")?;
            let method_params = self.convert_parameter_list(root, &method_param_list)?;
            let params = Self::collect_typed_params(
              [receiver_params, method_params].concat(),
              "method params",
            );
            let method_stmts = map_transposed_or_default(
              md.body(),
              "Error converting method body",
              vec![],
              |body| self.convert_block(root, &body),
            )?;
            let method_sig = il::MethodSig {
              name: with_ctx(md.name(), "Error converting method name")?
                .utf8_text(root.source().as_bytes())?
                .to_string(),
              params,
              id: None,
            };
            let ret_type = map_transposed_or_default(
              md.result(),
              "Error converting method return type",
              vec![],
              |result| self.convert_simple_type_parameter_list(root, &result),
            )?;
            let method_decl = il::DefineKind::Method {
              sig: method_sig,
              body: method_stmts,
              ret_type: ret_type,
            };
            let tag = self.source_info.register(*md.raw());
            Ok(vec![il::Statement::Define {
              kind: method_decl,
              scope_id: None,
              tag: Some(tag),
            }])
          }
          TopLevel::FunctionDeclaration(fd) => {
            let function_param_list =
              with_ctx(fd.parameters(), "Error converting function parameters")?;
            let function_params = self.convert_parameter_list(root, &function_param_list)?;
            let params = Self::collect_typed_params(function_params, "function params");
            let function_stmts = map_transposed_or_default(
              fd.body(),
              "Error converting function body",
              vec![],
              |body| self.convert_block(root, &body),
            )?;
            let function_sig = il::MethodSig {
              name: with_ctx(fd.name(), "Error converting function name")?
                .utf8_text(root.source().as_bytes())?
                .to_string(),
              params,
              id: None,
            };
            let ret_type = map_transposed_or_default(
              fd.result(),
              "Error converting function return type",
              vec![],
              |result| self.convert_simple_type_parameter_list(root, &result),
            )?;
            let method_decl = il::DefineKind::Method {
              sig: function_sig,
              body: function_stmts,
              ret_type: ret_type,
            };
            let tag = self.source_info.register(*fd.raw());
            Ok(vec![il::Statement::Define {
              kind: method_decl,
              scope_id: None,
              tag: Some(tag),
            }])
          }
          _ => {
            eprintln!("convert_toplevels: Unhandled node type: {:?}", top);
            Ok(vec![])
          }
        }
      })
      .collect::<Result<Vec<Vec<il::Statement>>>>()?;
    Ok(flatten_nested(results))
  }

  fn convert_block(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    block: &Block<'src>,
  ) -> Result<Vec<il::Statement>> {
    self.convert_optional_statement_list(
      root,
      block.statement_list(),
      "Error converting statement list",
      "Error converting block statement",
    )
  }

  fn convert_type(&mut self, root: &AstGrep<StrDoc<SupportLang>>, t: &Type<'src>) -> il::Type {
    match t {
      Type::SimpleType(st) => self.convert_simple_type(root, st),
      _ => {
        eprintln!("convert_type: Unhandled type: {:?}", t);
        il::Type::Any
      }
    }
  }

  fn convert_type_elem(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    te: &TypeElem<'src>,
  ) -> il::Type {
    te.types(&mut te.walk())
      .map(|t| self.convert_type(root, &t.unwrap()))
      .next()
      .unwrap_or(il::Type::Any)
  }

  fn convert_simple_type_parameter_list(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    pl: &SimpleType_ParameterList<'src>,
  ) -> Result<Vec<il::Type>> {
    match pl {
      SimpleType_ParameterList::SimpleType(st) => Ok(vec![self.convert_simple_type(root, &st)]),
      SimpleType_ParameterList::ParameterList(pl) => {
        self.convert_parameter_list(root, &pl).map(|vs| {
          vs.into_iter()
            .map(|v| match v {
              il::Value {
                kind: il::ValueKind::Ident(_name),
                extra:
                  ValueExtra {
                    type_declared: Some(t),
                    ..
                  },
                tag: _,
              } => t,
              _ => {
                eprintln!("Expected Ident value in function return type, got: {:?}", v);
                il::Type::Any
              }
            })
            .collect::<Vec<_>>()
        })
      }
    }
  }

  fn convert_simple_type(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    st: &SimpleType<'src>,
  ) -> il::Type {
    match st {
      SimpleType::TypeIdentifier(id) => {
        let name = id.utf8_text(root.source().as_bytes()).unwrap().to_string();
        if let Some(t) = BUILTIN_TYPES.get(name.as_str()) {
          t.clone()
        } else {
          il::Type::Named {
            name,
            generics: vec![],
          }
        }
      }
      SimpleType::GenericType(gt) => {
        let t = gt
          .r#type()
          .map(|t| match t {
            NegatedType_QualifiedType_TypeIdentifier::NegatedType(nt) => {
              self.convert_simple_type(root, &SimpleType::NegatedType(nt))
            }
            NegatedType_QualifiedType_TypeIdentifier::QualifiedType(qt) => {
              self.convert_simple_type(root, &SimpleType::QualifiedType(qt))
            }
            NegatedType_QualifiedType_TypeIdentifier::TypeIdentifier(id) => {
              self.convert_simple_type(root, &SimpleType::TypeIdentifier(id))
            }
          })
          .unwrap_or(il::Type::Any);
        let t_args = gt
          .type_arguments()
          .map(|args| {
            args
              .type_elems(&mut gt.walk())
              .map(|t| self.convert_type_elem(root, &t.unwrap()))
              .collect::<Vec<_>>()
          })
          .unwrap_or(vec![]);
        match t {
          il::Type::Named { name, generics } => il::Type::Named {
            name,
            generics: [generics, t_args].concat(),
          },
          _ => {
            eprintln!("convert_simple_type: Unhandled generic type: {:?}", gt);
            t
          }
        }
      }
      SimpleType::ChannelType(ct) => {
        let value_type = ct
          .value()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        let chan_name = match ct.utf8_text(root.source().as_bytes()) {
          Ok(name) if name.starts_with("chan<-") => "chan<-".to_string(),
          Ok(name) if name.starts_with("<-chan") => "<-chan".to_string(),
          Ok(_) => "chan".to_string(),
          Err(e) => {
            eprintln!("Error converting channel type: {:?}", e);
            "chan".to_string()
          }
        };
        il::Type::Named {
          name: chan_name,
          generics: vec![value_type],
        }
      }
      SimpleType::MapType(mt) => {
        let key_type = mt
          .key()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        let value_type = mt
          .value()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        il::Type::Named {
          name: "map".to_string(),
          generics: vec![key_type, value_type],
        }
      }
      SimpleType::SliceType(st) => {
        let elem_type = st
          .element()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        il::Type::Slice(Box::new(elem_type))
      }
      SimpleType::ArrayType(at) => {
        let elem_type = at
          .element()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        let (length_exp, length_stmts) = at
          .length()
          .map_err(|e| anyhow!("Error converting array length: {:?}", e))
          .and_then(|exp| self.convert_expression(root, &exp))
          .unwrap_or((
            il::Value {
              kind: il::ValueKind::IntLit(0),
              extra: ValueExtra::new(None, None),
              tag: None,
            },
            vec![],
          ));
        let length = match length_exp {
          il::Value {
            kind: il::ValueKind::IntLit(len),
            extra: ValueExtra { .. },
            tag: _,
          } => Some(len as usize),
          _ => {
            eprintln!(
              "Expected integer literal for array length, got: {:?}",
              length_exp
            );
            None
          }
        };
        if !length_stmts.is_empty() {
          eprintln!(
            "Array length conversion produced statements: {:?}",
            length_stmts
          );
        };
        il::Type::Array(Box::new(elem_type), length)
      }
      SimpleType::PointerType(pt) => {
        let elem_type = pt
          .r#type()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        il::Type::Pointer(Box::new(elem_type))
      }
      SimpleType::FunctionType(ft) => {
        let params = ft
          .parameters()
          .map_err(|e| anyhow!("Error converting function parameters: {:?}", e))
          .and_then(|pl| self.convert_parameter_list(root, &pl))
          .map(|vs| {
            vs.into_iter()
              .map(|v| match v {
                il::Value {
                  kind: il::ValueKind::Ident(_name),
                  extra:
                    ValueExtra {
                      type_declared: Some(t),
                      ..
                    },
                  tag: _,
                } => t,
                _ => {
                  eprintln!("Expected Ident value in function params, got: {:?}", v);
                  il::Type::Any
                }
              })
              .collect::<Vec<_>>()
          })
          .unwrap_or(vec![]);
        let ret_type = ft
          .result()
          .map(|ret| {
            {
              ret
                .map_err(|e| anyhow!("Error converting return type: {:?}", e))
                .and_then(|pl| self.convert_simple_type_parameter_list(root, &pl))
            }
            .unwrap_or(vec![])
          })
          .unwrap_or(vec![il::Type::Void]);
        il::Type::Function {
          params,
          ret: ret_type,
        }
      }
      SimpleType::NegatedType(nt) =>
      // Not supported yet.
      {
        nt.r#type()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any)
      }
      SimpleType::QualifiedType(qt) => {
        let name = qt
          .name()
          .map_err(|e| anyhow!("Error converting qualified type name: {:?}", e))
          .and_then(|id| {
            id.utf8_text(root.source().as_bytes())
              .map_err(|e| anyhow!("Error converting qualified type name text: {:?}", e))
              .map(|s| s.to_string())
          });
        let package = qt
          .package()
          .map_err(|e| anyhow!("Error converting qualified type package: {:?}", e))
          .and_then(|pkg| {
            pkg
              .utf8_text(root.source().as_bytes())
              .map_err(|e| anyhow!("Error converting qualified type package text: {:?}", e))
              .map(|s| s.to_string())
          });
        match (name, package) {
          (Ok(name), Ok(package)) => il::Type::Named {
            name: format!("{}.{}", package, name),
            generics: vec![],
          },
          _ => {
            eprintln!("convert_simple_type: Unhandled qualified type: {:?}", qt);
            il::Type::Any
          }
        }
      }
      _ => {
        eprintln!("convert_simple_type: Unhandled type: {:?}", st);
        il::Type::Any
      }
    }
  }

  fn convert_statement(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    st: &Statement<'src>,
  ) -> Result<Vec<il::Statement>> {
    match st {
      Statement::SimpleStatement(sst) => self.convert_simple_statement(root, sst),
      Statement::ConstDeclaration(cd) => {
        cd.const_specs(&mut cd.walk())
          .map(|spec| {
            let spec = with_ctx(spec, "Error converting const spec")?;
            let names = spec
              .names(&mut cd.walk())
              .filter_map(|name| match name {
                Ok(Comma_Identifier::Identifier(id)) => Some(Ok(
                  id.utf8_text(root.source().as_bytes()).unwrap().to_string(),
                )),
                Ok(Comma_Identifier::Comma(_)) => None,
                Err(e) => Some(Err(anyhow!("Error converting const name: {:?}", e))),
              })
              .collect::<Result<Vec<String>>>()?;
            let type_ = transpose_with_ctx(spec.r#type(), "Error converting const type")?
              .map(|t| self.convert_type(root, &t))
              .unwrap_or(il::Type::Any);
            if let Some(value_list) =
              transpose_with_ctx(spec.value(), "Error converting const value")?
            {
              let values = self.convert_expression_results(
                root,
                value_list.expressions(&mut cd.walk()),
                "Error converting const value expression",
              )?;
              let (values, stmts) = Self::split_expression_results(values);
              let tag = self.source_info.register(*spec.raw());
              // Create assignments for each variable-value pair
              Ok(
                [
                  stmts,
                  names
                    .into_iter()
                    .zip(values)
                    .map(|(name, value)| il::Statement::Assign {
                      left: il::Value {
                        kind: il::ValueKind::Ident(name),
                        extra: ValueExtra::new(Some(type_.clone()), None),
                        tag: None,
                      },
                      right: value,
                      tag: Some(tag),
                    })
                    .collect::<Vec<_>>(),
                ]
                .concat(),
              )
            } else {
              // Handle const declarations without initializers
              Err(anyhow!("Const declaration has no value"))
            }
          })
          .collect::<Result<Vec<Vec<_>>>>()
          .map(|inner| inner.into_iter().flatten().collect())
      }
      Statement::VarDeclaration(vd) => {
        let spec = vd.child().unwrap().as_var_spec().unwrap();
        let names = spec
          .names(&mut vd.walk())
          .map(|name| {
            name
              .utf8_text(root.source().as_bytes())
              .unwrap()
              .to_string()
          })
          .collect::<Vec<_>>();

        let type_ = spec
          .r#type()
          .and_then(|t| t.ok())
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);

        let tag = self.source_info.register(*spec.raw());

        if let Some(value_list) = transpose_with_ctx(spec.value(), "Error converting var value")? {
          let values = self.convert_expression_results(
            root,
            value_list.expressions(&mut vd.walk()),
            "Error converting var value expression",
          )?;
          let (values, stmts) = Self::split_expression_results(values);
          // Create assignments for each variable-value pair
          Ok(
            [
              stmts,
              names
                .into_iter()
                .zip(values)
                .map(|(name, value)| il::Statement::Define {
                  kind: il::DefineKind::Var {
                    name: name,
                    init: Some(value),
                    t: Some(type_.clone()),
                  },
                  scope_id: None,
                  tag: Some(tag),
                })
                .collect::<Vec<_>>(),
            ]
            .concat(),
          )
        } else {
          // Handle variable declarations without initializers
          Ok(
            names
              .iter()
              .map(|name| {
                let init = match type_ {
                  il::Type::Int
                  | il::Type::UInt
                  | il::Type::Byte
                  | il::Type::UByte
                  | il::Type::Short
                  | il::Type::UShort
                  | il::Type::Long
                  | il::Type::ULong
                  | il::Type::Float
                  | il::Type::Double => il::Value {
                    kind: il::ValueKind::IntLit(0),
                    extra: ValueExtra::new(Some(type_.clone()), None),
                    tag: None,
                  },
                  il::Type::Bool => il::Value {
                    kind: il::ValueKind::IntLit(0),
                    extra: ValueExtra::new(Some(type_.clone()), None),
                    tag: None,
                  },
                  il::Type::String => il::Value {
                    kind: il::ValueKind::StringLit("".to_string()),
                    extra: ValueExtra::new(Some(type_.clone()), None),
                    tag: None,
                  },
                  _ => il::Value {
                    kind: il::ValueKind::NullLit,
                    extra: ValueExtra::new(Some(type_.clone()), None),
                    tag: None,
                  },
                };
                il::Statement::Define {
                  kind: il::DefineKind::Var {
                    name: name.clone(),
                    init: Some(init),
                    t: Some(type_.clone()),
                  },
                  scope_id: None,
                  tag: Some(tag),
                }
              })
              .collect::<Vec<_>>(),
          )
        }
      }
      Statement::IfStatement(is) => {
        let (cond, cond_stmts) = self.convert_expression(root, &is.condition().unwrap())?;
        let body = is.consequence().unwrap();
        let else_body = is.alternative();
        let then_stmts = self.convert_block(root, &body)?;
        let else_stmts = if let Some(Ok(else_body)) = else_body {
          match else_body {
            Block_IfStatement::Block(block) => self.convert_block(root, &block)?,
            Block_IfStatement::IfStatement(if_stmt) => {
              self.convert_statement(root, &Statement::IfStatement(if_stmt))?
            }
          }
        } else {
          vec![]
        };
        let src_node = is.raw();
        let tag = self.source_info.register(*src_node);
        Ok(
          [
            cond_stmts,
            vec![il::Statement::If {
              condition: cond,
              then_stmts,
              else_stmts,
              tag: Some(tag),
            }],
          ]
          .concat(),
        )
      }
      Statement::ForStatement(fs) => {
        let body = with_ctx(fs.body(), "Error converting for body")?;
        let stmts = self.convert_block(root, &body)?;
        let tag = self.source_info.register(*fs.raw());
        match fs.other() {
          None => {
            // Simple for loop
            Ok(vec![il::Statement::For {
              init: vec![],
              condition: StatementValue {
                statements: vec![],
                result: Some(il::Value {
                  kind: il::ValueKind::IntLit(1),
                  extra: ValueExtra::new(None, None),
                  tag: None,
                }),
              },
              update: vec![],
              body: stmts,
              tag: Some(tag),
            }])
          }
          Some(Ok(Expression_ForClause_RangeClause::Expression(for_clause))) => {
            // For loop with condition
            let (cond, cond_stmts) = self.convert_expression(root, &for_clause)?;
            Ok(vec![il::Statement::For {
              init: vec![],
              condition: StatementValue {
                statements: cond_stmts,
                result: Some(cond),
              },
              update: vec![],
              body: stmts,
              tag: Some(tag),
            }])
          }
          Some(Ok(Expression_ForClause_RangeClause::ForClause(for_clause))) => {
            let init_stmts = map_transposed_or_default(
              for_clause.initializer(),
              "Error converting for initializer",
              vec![],
              |initializer| self.convert_simple_statement(root, &initializer),
            )?;
            let (cond, cond_stmts) = map_transposed_or_default(
              for_clause.condition(),
              "Error converting for condition",
              Self::default_true_condition(),
              |condition| self.convert_expression(root, &condition),
            )?;
            let update_stmts = map_transposed_or_default(
              for_clause.update(),
              "Error converting for update",
              vec![],
              |update| self.convert_simple_statement(root, &update),
            )?;
            Ok(vec![il::Statement::For {
              init: init_stmts,
              condition: StatementValue {
                statements: cond_stmts,
                result: Some(cond),
              },
              update: update_stmts,
              body: stmts,
              tag: Some(tag),
            }])
          }
          _ => Ok(vec![]), // TODO: RangeClause handling
        }
      }
      Statement::TypeSwitchStatement(tss) => {
        let value = with_ctx(tss.value(), "Error converting type switch value")?;
        let (expr, expr_stmts) = self.convert_expression(root, &value)?;
        let mut cases = Vec::new();
        let others = tss
          .others(&mut tss.walk())
          .map(|x| with_ctx(x, "Error converting type switch case"))
          .collect::<Result<Vec<_>>>()?;
        let (err, default, cases) = others.into_iter().fold(
          (None, None, &mut cases),
          |(err, acc_default, acc_stmts), child| match child {
            DefaultCase_TypeCase::DefaultCase(dc) => {
              let stmts = self.convert_optional_statement_list(
                root,
                dc.statement_list(),
                "Error converting type switch default case statement list",
                "Error converting type switch default case statement",
              );
              match stmts {
                Ok(stmts) => (err, Some(stmts), acc_stmts),
                Err(e) => (Some(e), acc_default, acc_stmts),
              }
            }
            DefaultCase_TypeCase::TypeCase(tc) => {
              let stmts = self.convert_optional_statement_list(
                root,
                tc.statement_list(),
                "Error converting type switch case statement list",
                "Error converting type switch case statement",
              );
              let types = tc
                .types(&mut tc.walk())
                .filter_map(|t| match t {
                  Ok(Comma_Type::Type(t)) => Some(Ok(self.convert_type(root, &t))),
                  Ok(Comma_Type::Comma(_)) => None,
                  Err(e) => Some(Err(anyhow!("Error converting type switch case: {:?}", e))),
                })
                .collect::<Result<Vec<_>>>();
              match (stmts, types) {
                (Ok(stmts), Ok(types)) => {
                  let mut result = Vec::new();
                  let len = types.len();
                  for (i, t) in types.into_iter().enumerate() {
                    if i < len - 1 {
                      result.push((
                        StatementValue {
                          statements: vec![],
                          result: Some(il::Value {
                            kind: il::ValueKind::TypeLit(t),
                            extra: ValueExtra::new(None, None),
                            tag: None,
                          }),
                        },
                        vec![],
                      ));
                    } else {
                      let mut stmts = stmts.clone();
                      stmts.push(il::Statement::Break);
                      result.push((
                        StatementValue {
                          statements: vec![],
                          result: Some(il::Value {
                            kind: il::ValueKind::TypeLit(t),
                            extra: ValueExtra::new(None, None),
                            tag: None,
                          }),
                        },
                        stmts,
                      ));
                    }
                  }
                  acc_stmts.extend(result);
                  (err, acc_default, acc_stmts)
                }
                (Err(e), _) => (Some(e), acc_default, acc_stmts),
                (_, Err(e)) => (Some(e), acc_default, acc_stmts),
              }
            }
          },
        );
        let tag = self.source_info.register(*tss.raw());
        match err {
          None => Ok(
            [vec![il::Statement::Switch {
              value: StatementValue {
                statements: expr_stmts,
                result: Some(expr),
              },
              cases: cases.clone(),
              default,
              tag: Some(tag),
            }]]
            .concat(),
          ),
          Some(e) => Err(e),
        }
      }
      Statement::ExpressionSwitchStatement(ess) => {
        let initializer = transpose_with_ctx(
          ess.initializer(),
          "Error converting expression switch initializer",
        )?;
        let init_stmts = initializer
          .map(|init| self.convert_simple_statement(root, &init))
          .transpose()?;
        let value = transpose_with_ctx(ess.value(), "Error converting expression switch value")?;
        let expr = value
          .map(|v| self.convert_expression(root, &v))
          .transpose()?;
        let others = ess
          .others(&mut ess.walk())
          .map(|x| with_ctx(x, "Error converting expression switch case"))
          .collect::<Result<Vec<_>>>()?;
        let (err, default, cases) = others.into_iter().fold(
          (None, None, Vec::new()),
          |(err, default, mut cases), case| match case {
            DefaultCase_ExpressionCase::ExpressionCase(ec) => {
              let stmts = self.convert_optional_statement_list(
                root,
                ec.statement_list(),
                "Error converting expression switch case statement list",
                "Error converting expression switch case statement",
              );
              let expr =
                with_ctx(ec.value(), "Error converting expression switch case").and_then(|es| {
                  self.convert_expression_results(
                    root,
                    es.expressions(&mut ec.walk()),
                    "Error converting expression switch case",
                  )
                });
              match (stmts, expr) {
                (Ok(stmts), Ok(expr)) => {
                  for (e, e_stmts) in expr {
                    cases.push((
                      StatementValue {
                        statements: e_stmts,
                        result: Some(e),
                      },
                      stmts.clone(),
                    ));
                  }
                  (err, default, cases)
                }
                (Err(e), _) => (Some(e), default, cases),
                (_, Err(e)) => (Some(e), default, cases),
              }
            }
            DefaultCase_ExpressionCase::DefaultCase(dc) => {
              let stmts = self.convert_optional_statement_list(
                root,
                dc.statement_list(),
                "Error converting expression switch default case statement list",
                "Error converting expression switch default case statement",
              );
              match stmts {
                Ok(stmts) => (err, Some(stmts), cases),
                Err(e) => (Some(e), default, cases),
              }
            }
          },
        );
        let tag = self.source_info.register(*ess.raw());
        match err {
          None => Ok(
            [vec![il::Statement::Switch {
              value: StatementValue {
                statements: init_stmts
                  .unwrap_or(vec![])
                  .into_iter()
                  .chain(
                    expr
                      .clone()
                      .map(|(_, expr_stmts)| expr_stmts)
                      .unwrap_or(vec![])
                      .into_iter(),
                  )
                  .collect(),
                result: expr.map(|(expr, _)| expr),
              },
              cases: cases.clone(),
              default,
              tag: Some(tag),
            }]]
            .concat(),
          ),
          Some(e) => Err(e),
        }
      }
      Statement::ReturnStatement(rs) => {
        let values =
          match transpose_with_ctx(rs.expression_list(), "Error converting return statement")? {
            Some(expr_list) => self.convert_expression_results(
              root,
              expr_list.expressions(&mut expr_list.walk()),
              "Error converting return expression",
            )?,
            None => vec![],
          };
        let (values, stmts) = Self::split_expression_results(values);
        let tag = self.source_info.register(*rs.raw());
        Ok(
          [
            stmts,
            vec![il::Statement::Return {
              // TODO: Handle multiple return values
              value: values.first().cloned(),
              tag: Some(tag),
            }],
          ]
          .concat(),
        )
      }
      Statement::BreakStatement(_) => Ok(vec![il::Statement::Break]),
      Statement::ContinueStatement(_) => Ok(vec![il::Statement::Continue]),
      _ => {
        eprintln!("convert_statement: Unhandled node type: {:?}", st);
        Ok(vec![])
      }
    }
  }

  fn convert_simple_statement(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    sst: &SimpleStatement<'src>,
  ) -> Result<Vec<il::Statement>> {
    match sst {
      SimpleStatement::ShortVarDeclaration(s) => {
        let mut left_values = Vec::new();
        let mut left_stmts = Vec::new();
        if let Ok(left_list) = s.left() {
          let left = self.convert_expression_results(
            root,
            left_list.expressions(&mut s.walk()),
            "Error converting short var declaration left expression",
          )?;
          (left_values, left_stmts) = Self::split_expression_results(left);
        }
        let mut right_values = Vec::new();
        let mut right_stmts = Vec::new();
        if let Ok(right_list) = s.right() {
          let right = self.convert_expression_results(
            root,
            right_list.expressions(&mut s.walk()),
            "Error converting short var declaration right expression",
          )?;
          (right_values, right_stmts) = Self::split_expression_results(right);
        }
        let tag = self.source_info.register(*s.raw());
        // Create assignments for each variable-value pair
        Ok(
          [
            left_stmts,
            right_stmts,
            left_values
              .into_iter()
              .zip(right_values)
              .map(|(left, right)| {
                let implicit_type = right.extra.type_declared.clone();
                il::Statement::Define {
                  kind: il::DefineKind::Var {
                    name: match left.kind {
                      il::ValueKind::Ident(name) => name,
                      _ => {
                        eprintln!(
                          "Expected Ident on left side of short var declaration, got: {:?}",
                          left
                        );
                        "_".to_string()
                      }
                    },
                    init: Some(right),
                    t: implicit_type,
                  },
                  scope_id: None,
                  tag: Some(tag),
                }
              })
              .collect::<Vec<_>>(),
          ]
          .concat(),
        )
      }
      SimpleStatement::ExpressionStatement(es) => {
        let (expr, stmts) = self.convert_expression(root, &es.expression().unwrap())?;
        let tag = self.source_info.register(*es.raw());
        Ok(
          [
            stmts,
            vec![il::Statement::Assign {
              left: il::Value {
                kind: il::ValueKind::Ident("_".to_string()),
                extra: ValueExtra::new(None, None),
                tag: None,
              },
              right: expr,
              tag: Some(tag),
            }],
          ]
          .concat(),
        )
      }
      SimpleStatement::IncStatement(is) => {
        let tag = self.source_info.register(*is.raw());
        with_ctx(is.expression(), "Error converting increment statement")
          .and_then(|expr| self.convert_expression(root, &expr))
          .map(|(value, stmts)| {
            [
              stmts,
              vec![il::Statement::Assign {
                left: value.clone(),
                right: il::Value {
                  kind: il::ValueKind::Exp(il::Expr::BinOp {
                    left: Box::new(value),
                    right: Box::new(il::Value {
                      kind: il::ValueKind::IntLit(1),
                      extra: ValueExtra::new(None, None),
                      tag: None,
                    }),
                    op: il::BinaryOp::Add,
                  }),
                  extra: ValueExtra::new(None, None),
                  tag: None,
                },
                tag: Some(tag),
              }],
            ]
            .concat()
          })
      }
      SimpleStatement::DecStatement(ds) => {
        let tag = self.source_info.register(*ds.raw());
        with_ctx(ds.expression(), "Error converting decrement statement")
          .and_then(|expr| self.convert_expression(root, &expr))
          .map(|(value, stmts)| {
            [
              stmts,
              vec![il::Statement::Assign {
                left: value.clone(),
                right: il::Value {
                  kind: il::ValueKind::Exp(il::Expr::BinOp {
                    left: Box::new(value),
                    right: Box::new(il::Value {
                      kind: il::ValueKind::IntLit(1),
                      extra: ValueExtra::new(None, None),
                      tag: None,
                    }),
                    op: il::BinaryOp::Sub,
                  }),
                  extra: ValueExtra::new(None, None),
                  tag: None,
                },
                tag: Some(tag),
              }],
            ]
            .concat()
          })
      }
      SimpleStatement::AssignmentStatement(asg) => {
        let mut left_values = Vec::new();
        let mut left_stmts = Vec::new();
        if let Ok(left_list) = asg.left() {
          let left = self.convert_expression_results(
            root,
            left_list.expressions(&mut asg.walk()),
            "Error converting assignment left expression",
          )?;
          (left_values, left_stmts) = Self::split_expression_results(left);
        }
        let mut right_values = Vec::new();
        let mut right_stmts = Vec::new();
        if let Ok(right_list) = asg.right() {
          let right = self.convert_expression_results(
            root,
            right_list.expressions(&mut asg.walk()),
            "Error converting assignment right expression",
          )?;
          (right_values, right_stmts) = Self::split_expression_results(right);
        }
        let tag = self.source_info.register(*asg.raw());
        // Create assignments for each variable-value pair
        Ok(
          [
            left_stmts,
            right_stmts,
            left_values
              .into_iter()
              .zip(right_values)
              .map(|(left, right)| il::Statement::Assign {
                left,
                right,
                tag: Some(tag),
              })
              .collect::<Vec<_>>(),
          ]
          .concat(),
        )
      }
      _ => {
        eprintln!("convert_simple_statement: Unhandled node type: {:?}", sst);
        Ok(vec![])
      }
    }
  }

  fn convert_binary_operator(&mut self, op: &BinaryOp) -> il::BinaryOp {
    match op {
      BinaryOp::NotEq(_) => il::BinaryOp::Neq,
      BinaryOp::Mod(_) => il::BinaryOp::Mod,
      BinaryOp::And(_) => il::BinaryOp::And,
      BinaryOp::AndAnd(_) => il::BinaryOp::AndAnd,
      BinaryOp::BitXor(_) => il::BinaryOp::Xor,
      BinaryOp::Mul(_) => il::BinaryOp::Mul,
      BinaryOp::Add(_) => il::BinaryOp::Add,
      BinaryOp::Sub(_) => il::BinaryOp::Sub,
      BinaryOp::Div(_) => il::BinaryOp::Div,
      BinaryOp::Lt(_) => il::BinaryOp::Lt,
      BinaryOp::LtEq(_) => il::BinaryOp::Lte,
      BinaryOp::EqEq(_) => il::BinaryOp::Eq,
      BinaryOp::Gt(_) => il::BinaryOp::Gt,
      BinaryOp::GtEq(_) => il::BinaryOp::Gte,
      BinaryOp::GtGt(_) => il::BinaryOp::Shr,
      BinaryOp::LtLt(_) => il::BinaryOp::Shl,
      BinaryOp::Or(_) => il::BinaryOp::Or,
      BinaryOp::OrOr(_) => il::BinaryOp::OrOr,
      BinaryOp::AndBitXor(_) => il::BinaryOp::Xor,
    }
  }

  fn convert_unary_operator(&mut self, op: &UnaryOp) -> il::UnaryOp {
    match op {
      UnaryOp::Not(_) => il::UnaryOp::Not,
      UnaryOp::And(_) => il::UnaryOp::And,
      UnaryOp::Mul(_) => il::UnaryOp::Mul,
      UnaryOp::Add(_) => il::UnaryOp::Add,
      UnaryOp::Sub(_) => il::UnaryOp::Sub,
      UnaryOp::LtSub(_) => il::UnaryOp::LtSub,
      UnaryOp::BitXor(_) => il::UnaryOp::BitXor,
    }
  }

  fn convert_literal_element(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    node: &LiteralElement<'src>,
  ) -> Result<(il::Value, Vec<il::Statement>)> {
    with_ctx(node.child(), "Error converting literal element child").and_then(|child| match child {
      Expression_LiteralValue::Expression(e) => self.convert_expression(root, &e),
      Expression_LiteralValue::LiteralValue(le) => self.convert_literal_value(root, &le),
    })
  }

  fn convert_literal_value(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    node: &LiteralValue<'src>,
  ) -> Result<(il::Value, Vec<il::Statement>)> {
    let elements = node
      .children(&mut node.walk())
      .map(|child| {
        with_ctx(child, "Error converting literal element child").and_then(|e| match e {
          KeyedElement_LiteralElement::KeyedElement(ke) => {
            let (key, key_stmts) = with_ctx(ke.key(), "Error converting keyed element key")
              .and_then(|k| self.convert_literal_element(root, &k))?;
            let (value, value_stmts) = with_ctx(ke.value(), "Error converting keyed element value")
              .and_then(|v| self.convert_literal_element(root, &v))?;
            Ok((
              (Some(key), value),
              key_stmts.into_iter().chain(value_stmts).collect(),
            ))
          }
          KeyedElement_LiteralElement::LiteralElement(le) => {
            let (value, stmts) = self.convert_literal_element(root, &le)?;
            Ok(((None, value), stmts))
          }
        })
      })
      .collect::<Result<Vec<_>>>()?;
    let (kvs, stmts): (Vec<(Option<il::Value>, il::Value)>, Vec<Vec<il::Statement>>) =
      elements.into_iter().unzip();
    let tag = self.source_info.register(*node.raw());
    let items = kvs
      .into_iter()
      .map(|(key, value)| {
        if let Some(k) = key {
          il::CompositeItem::KV(k, value)
        } else {
          il::CompositeItem::Value(value)
        }
      })
      .collect();
    Ok((
      il::Value {
        kind: il::ValueKind::Exp(il::Expr::Composite { elements: items }),
        extra: ValueExtra::new(None, None),
        tag: Some(tag),
      },
      stmts.into_iter().flatten().collect(),
    ))
  }

  fn convert_expression(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    node: &Expression<'src>,
  ) -> Result<(il::Value, Vec<il::Statement>)> {
    match node {
      Expression::Identifier(ident) => {
        let name = ident.utf8_text(root.source().as_bytes())?;
        let tag = self.source_info.register(*ident.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::Ident(name.to_string()),
            extra: ValueExtra::new(None, None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::IntLiteral(lit) => {
        let value = lit.utf8_text(root.source().as_bytes())?.parse()?;
        let tag = self.source_info.register(*lit.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::IntLit(value),
            extra: ValueExtra::new(Some(il::Type::Int), None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::FloatLiteral(lit) => {
        let value = lit.utf8_text(root.source().as_bytes())?.parse()?;
        let tag = self.source_info.register(*lit.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::FloatLit(value),
            extra: ValueExtra::new(Some(il::Type::Float), None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::InterpretedStringLiteral(lit) => {
        let text = lit
          .utf8_text(root.source().as_bytes())?
          .trim_matches('"')
          .to_string();
        let tag = self.source_info.register(*lit.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::StringLit(text),
            extra: ValueExtra::new(None, None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::RawStringLiteral(lit) => {
        let text = lit
          .utf8_text(root.source().as_bytes())?
          .trim_matches('`')
          .to_string();
        let tag = self.source_info.register(*lit.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::StringLit(text),
            extra: ValueExtra::new(None, None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::Nil(n) => {
        let tag = self.source_info.register(*n.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::NullLit,
            extra: ValueExtra::new(None, None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::BinaryExpression(be) => {
        let (left, left_stmts) = self.convert_expression(root, &be.left().unwrap())?;
        let (right, right_stmts) = self.convert_expression(root, &be.right().unwrap())?;
        let bin_op = self.convert_binary_operator(&be.operator().unwrap());
        let tag = self.source_info.register(*be.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::Exp(il::Expr::BinOp {
              left: Box::new(left),
              right: Box::new(right),
              op: bin_op,
            }),
            extra: ValueExtra::new(None, None),
            tag: Some(tag),
          },
          [left_stmts, right_stmts].concat(),
        ))
      }
      Expression::UnaryExpression(ue) => {
        let op =
          self.convert_unary_operator(&with_ctx(ue.operator(), "Error converting unary operator")?);
        let (value, stmts) = self.convert_expression(
          root,
          &with_ctx(ue.operand(), "Error converting unary operand")?,
        )?;
        if op == il::UnaryOp::Mul {
          return Ok((
            il::Value {
              kind: il::ValueKind::Exp(il::Expr::Deref {
                value: Box::new(value),
              }),
              extra: ValueExtra::new(None, None),
              tag: None,
            },
            stmts,
          ));
        }
        let t = match op {
          il::UnaryOp::And => Some(il::Type::Pointer(Box::new(
            value.extra.type_declared.clone().unwrap_or(il::Type::Any),
          ))),
          _ => None,
        };
        Ok((
          il::Value {
            kind: il::ValueKind::Exp(il::Expr::UnOp {
              value: Box::new(value),
              op,
            }),
            extra: ValueExtra::new(t, None),
            tag: None,
          },
          stmts,
        ))
      }
      Expression::True(_) => {
        let tag = self.source_info.register(*node.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::IntLit(1),
            extra: ValueExtra::new(Some(il::Type::Bool), None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::False(_) => {
        let tag = self.source_info.register(*node.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::IntLit(0),
            extra: ValueExtra::new(Some(il::Type::Bool), None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::SelectorExpression(se) => {
        let (base, base_stmts) = self.convert_expression(root, &se.operand().unwrap())?;
        let field = with_ctx(se.field(), "Error converting field")?
          .utf8_text(root.source().as_bytes())?
          .to_string();
        Ok((
          il::Value {
            kind: il::ValueKind::Exp(il::Expr::DotAccess {
              base: Box::new(base),
              field,
            }),
            extra: ValueExtra::new(None, None),
            tag: None,
          },
          base_stmts,
        ))
      }
      Expression::CallExpression(ce) => {
        let function = with_ctx(ce.function(), "Error converting call expression")?;
        let (func_value, func_stmts) = self.convert_expression(root, &function)?;
        let args = with_ctx(ce.arguments(), "Error converting call expression")?
          .children(&mut ce.walk())
          .map(|e| match e {
            Ok(Expression_Type_VariadicArgument::Expression(e)) => {
              self.convert_expression(root, &e)
            }
            Ok(Expression_Type_VariadicArgument::VariadicArgument(e)) => {
              let expr = with_ctx(e.expression(), "Error converting call expression")?;
              self.convert_expression(root, &expr)
            }
            Ok(Expression_Type_VariadicArgument::Type(t)) => {
              let t = self.convert_type(root, &t);
              Ok((
                il::Value {
                  kind: il::ValueKind::TypeLit(t),
                  extra: ValueExtra::new(None, None),
                  tag: None,
                },
                vec![],
              ))
            }
            Err(e) => Err(anyhow!(
              "Error converting call expression argument: {:?}",
              e
            )),
          })
          .collect::<Result<Vec<_>>>()?;
        let (args_values, args_stmts): (Vec<il::Value>, Vec<Vec<il::Statement>>) =
          args.into_iter().unzip();
        let (tmp_var, _) = self.get_tmp_var(None);
        let src_node = ce.raw();
        let tag = self.source_info.register(*src_node);
        let invoke_stmt = il::Statement::Invoke {
          kind: il::InvokeKind::Static,
          callee: func_value,
          args: args_values,
          ret_type: il::Type::Any, // TODO: Determine the actual return type
          ret_loc: Some(tmp_var.clone()),
          tag: Some(tag),
        };
        Ok((
          tmp_var,
          [func_stmts, flatten_nested(args_stmts), vec![invoke_stmt]].concat(),
        ))
      }
      Expression::FuncLiteral(fl) => {
        let parameter_list = with_ctx(fl.parameters(), "Error converting function parameters")?;
        let params = self.convert_parameter_list(root, &parameter_list)?;
        let params = Self::collect_typed_params(params, "function literal params");
        let ret_type = map_transposed_or_default(
          fl.result(),
          "Error converting function result type",
          vec![],
          |result| self.convert_simple_type_parameter_list(root, &result),
        )?;
        let (method_var, method_name) = self.get_tmp_var(Some(il::Type::Function {
          params: params.iter().map(|(t, _)| t.clone()).collect(),
          ret: ret_type.clone(),
        }));
        let method_sig = il::MethodSig {
          name: method_name,
          params,
          id: None,
        };
        let method_decl = il::DefineKind::Method {
          sig: method_sig,
          body: with_ctx(fl.body(), "Error converting function body")
            .and_then(|body| self.convert_block(root, &body))?,
          ret_type,
        };
        let tag = self.source_info.register(*fl.raw());
        Ok((
          il::Value {
            kind: method_var.kind,
            extra: method_var.extra,
            tag: Some(tag),
          },
          vec![il::Statement::Define {
            kind: method_decl,
            scope_id: None,
            tag: Some(tag),
          }],
        ))
      }
      Expression::CompositeLiteral(cl) => {
        let body = with_ctx(cl.body(), "Error converting composite literal body")?;
        let (elems, stmts) = self.convert_literal_value(root, &body)?;
        let type_ = with_ctx(cl.r#type(), "Error converting composite literal type")
          .and_then(|t| self.convert_composite_literal_type(root, &t))?;
        let tag = self.source_info.register(*cl.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::CompositeLit(type_.clone(), Box::new(elems)),
            extra: ValueExtra::new(Some(type_), None),
            tag: Some(tag),
          },
          stmts,
        ))
      }
      _ => {
        eprintln!("convert_expression: Unhandled value type: {:?}", node);
        Ok((
          il::Value {
            kind: il::ValueKind::NullLit,
            extra: ValueExtra::new(None, None),
            tag: None,
          },
          vec![],
        ))
      }
    }
  }

  fn convert_composite_literal_type(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    node: &CompositeLiteralType<'src>,
  ) -> Result<il::Type> {
    match node {
      CompositeLiteralType::TypeIdentifier(ti) => {
        let type_name = ti.utf8_text(root.source().as_bytes())?.to_string();
        Ok(il::Type::Named {
          name: type_name,
          generics: vec![],
        })
      }
      _ => {
        eprintln!(
          "convert_composite_literal_type: Unhandled type node: {:?}",
          node
        );
        Ok(il::Type::Any)
      }
    }
  }
}
