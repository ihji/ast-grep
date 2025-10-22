use std::collections::HashMap;

use string_interner::symbol::DefaultSymbol as StrSymbol;
use string_interner::DefaultStringInterner;

struct NamingContext {
  interner: DefaultStringInterner,
  scopes: Vec<Scope>,
  symbols: Vec<Symbol>,
  scope_stack: Vec<ScopeId>,
}

type ScopeId = u32;
type SymbolId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Namespace {
  Type,
  Value,
  Method,
  Macro,
}

enum Visibility {
  Public,
  Module,
  Package,
  Private,
}

struct Symbol {
  id: SymbolId,
  name: StrSymbol,
  ns: Namespace,
  owner_scope: ScopeId,
  visibility: Visibility,
  tag: Option<usize>,
}

enum ScopeKind {
  File,
  Package,
  Module,
  Function,
  Block,
}

struct Scope {
  id: ScopeId,
  kind: ScopeKind,
  parent: Option<ScopeId>,
  children: Vec<ScopeId>,
  symbols: HashMap<(StrSymbol, Namespace), Vec<SymbolId>>,
}

impl NamingContext {
  fn new() -> NamingContext {
    NamingContext {
      interner: DefaultStringInterner::new(),
      scopes: Vec::new(),
      symbols: Vec::new(),
      scope_stack: Vec::new(),
    }
  }

  fn enter_scope(&mut self, kind: ScopeKind) -> ScopeId {
    let parent = self.scope_stack.last().cloned();
    let scope_id = self.scopes.len() as ScopeId;
    let scope = Scope {
      id: scope_id,
      kind,
      parent,
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

  fn exit_scope(&mut self) {
    self.scope_stack.pop();
  }

  fn register_symbol(
    &mut self,
    name: &str,
    ns: Namespace,
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
      ns: ns,
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
}
