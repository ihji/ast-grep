use std::collections::HashMap;
use std::fmt::Debug;

use string_interner::symbol::DefaultSymbol as StrSymbol;
use string_interner::DefaultStringInterner;

use crate::il;

pub struct NamingContext {
  interner: DefaultStringInterner,
  scopes: Vec<Scope>,
  symbols: Vec<Symbol>,
  scope_stack: Vec<ScopeId>,
}

pub type ScopeId = u32;
pub type SymbolId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Namespace {
  Type,
  Value,
  Method,
  Macro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
  Public,
  Module,
  Package,
  Private,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclKind {
  Var(Option<il::Type>),
  Function,
  Class,
  Struct,
  Enum,
  Trait,
  Impl,
  Interface,
  Module,
  Package,
}

#[derive(Debug)]
pub struct Symbol {
  pub id: SymbolId,
  name: StrSymbol,
  ns: Namespace,
  pub kind: DeclKind,
  owner_scope: ScopeId,
  visibility: Visibility,
  tag: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
  File,
  Package,
  Module,
  Function,
  Block,
}

#[derive(Debug)]
struct Scope {
  id: ScopeId,
  name: Option<StrSymbol>,
  kind: ScopeKind,
  parent: Option<ScopeId>,
  children: Vec<ScopeId>,
  symbols: HashMap<(StrSymbol, Namespace), Vec<SymbolId>>,
}

impl NamingContext {
  pub fn new() -> NamingContext {
    NamingContext {
      interner: DefaultStringInterner::new(),
      scopes: Vec::new(),
      symbols: Vec::new(),
      scope_stack: Vec::new(),
    }
  }

  pub fn clear_scope_stack(&mut self) {
    self.scope_stack.clear();
  }

  pub fn enter_scope(&mut self, kind: ScopeKind, name: Option<&str>) -> ScopeId {
    let parent = self.scope_stack.last().cloned();
    let scope_id = self.scopes.len() as ScopeId;
    let scope = Scope {
      id: scope_id,
      kind,
      parent,
      name: name.map(|n| self.interner.get_or_intern(n)),
      children: Vec::new(),
      symbols: HashMap::new(),
    };
    if let Some(parent_id) = parent {
      self.scopes[parent_id as usize].children.push(scope_id);
    }
    self.scopes.push(scope);
    self.scope_stack.push(scope_id);
    scope_id
  }

  pub fn exit_scope(&mut self) {
    self.scope_stack.pop();
  }

  pub fn register_symbol(
    &mut self,
    name: &str,
    ns: Namespace,
    kind: DeclKind,
    visibility: Visibility,
    tag: Option<usize>,
  ) -> SymbolId {
    let name_sym = self.interner.get_or_intern(name);
    let owner_scope = self
      .scope_stack
      .last()
      .expect("No scope to register symbol in");
    let symbol_id = self.symbols.len() as SymbolId;
    let symbol = Symbol {
      id: symbol_id,
      name: name_sym,
      ns,
      kind,
      owner_scope: *owner_scope,
      visibility,
      tag,
    };
    self.symbols.push(symbol);
    let scope = &mut self.scopes[*owner_scope as usize];
    scope
      .symbols
      .entry((name_sym, ns))
      .or_insert_with(Vec::new)
      .push(symbol_id);
    symbol_id
  }

  pub fn lookup_symbol(
    &mut self,
    name: &str,
    ns: Namespace,
    starting_scope: Option<ScopeId>,
  ) -> Option<&Symbol> {
    let name_sym = self.interner.get_or_intern(name);
    let mut current_scope_id = starting_scope;
    while let Some(scope_id) = current_scope_id {
      let scope = &self.scopes[scope_id as usize];
      if let Some(symbol_ids) = scope.symbols.get(&(name_sym, ns)) {
        if let Some(&symbol_id) = symbol_ids.last() {
          return Some(&self.symbols[symbol_id as usize]);
        }
      }
      current_scope_id = scope.parent;
    }
    None
  }
}

impl Debug for NamingContext {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    writeln!(f, "NamingContext {{")?;
    writeln!(f, "  Scopes:")?;
    for scope in &self.scopes {
      let scope_name = scope
        .name
        .and_then(|sym| self.interner.resolve(sym))
        .unwrap_or("<unnamed>");
      writeln!(
        f,
        "    Scope ID: {}, Kind: {:?}, Name: {:?}, Parent: {:?}, Children: {:?}",
        scope.id, scope.kind, scope_name, scope.parent, scope.children
      )?;
      for ((name_sym, _ns), symbol_ids) in &scope.symbols {
        let sym_name = self.interner.resolve(*name_sym).unwrap();
        for symbol_id in symbol_ids {
          let symbol = &self.symbols[*symbol_id as usize];
          writeln!(
            f,
            "      Symbol ID: {}, Name: {}, Namespace: {:?}, Kind: {:?}, Visibility: {:?}, Tag: {:?}",
            symbol.id, sym_name, symbol.ns, symbol.kind, symbol.visibility, symbol.tag
          )?;
        }
      }
    }
    writeln!(f, "}}")
  }
}
