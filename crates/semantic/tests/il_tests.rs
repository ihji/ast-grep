use std::path::PathBuf;

use ast_grep_semantic::il::{
  BinaryOp, DefineKind, Expr, InvokeKind, MethodSig, PrettyPrinter, Program, Statement, Type,
  UnaryOp, Value, ValueExtra, ValueKind,
};

#[test]
fn test_simple_program() {
  let mut program = Program::new(PathBuf::new());

  // Create a simple program:
  // x = 42
  // y = foo(x)
  // if y then {
  //   return y
  // } else {
  //   goto 2
  // }

  let x = Value {
    kind: ValueKind::Ident("x".to_string()),
    extra: ValueExtra::new(Some(Type::Int), None),
    tag: None,
  };
  let y = Value {
    kind: ValueKind::Ident("y".to_string()),
    extra: ValueExtra::new(Some(Type::Int), None),
    tag: None,
  };
  let _forty_two = Value {
    kind: ValueKind::IntLit(42),
    extra: ValueExtra::new(Some(Type::Int), None),
    tag: None,
  };
  let forty_two = Value {
    kind: ValueKind::IntLit(42),
    extra: ValueExtra::new(Some(Type::Int), None),
    tag: None,
  };

  program.add_statement(Statement::Assign {
    left: x.clone(),
    right: forty_two,
    tag: None,
  });

  program.add_statement(Statement::Invoke {
    kind: InvokeKind::Static,
    callee: Value {
      kind: ValueKind::Ident("foo".to_string()),
      extra: ValueExtra::new(Some(Type::Int), None),
      tag: None,
    },
    args: vec![x.clone()],
    ret_type: Type::Int,
    ret_loc: Some(y.clone()),
    tag: None,
  });

  program.add_statement(Statement::If {
    condition: y.clone(),
    then_stmts: vec![Statement::Return {
      value: Some(y.clone()),
      tag: None,
    }],
    else_stmts: vec![Statement::Goto { target: 2 }],
    tag: None,
  });

  let expected = r#"0: x: int = 42
1: y: int = static foo: int(x: int) : int
2: if y: int then {
  0: return y: int
} else {
  0: goto 2
}"#;

  assert_eq!(PrettyPrinter::print_program(&program), expected);
}

#[test]
fn test_function_call() {
  let mut program = Program::new(PathBuf::new());

  // Create a program with nested function calls:
  // x = foo(bar(42))

  let x = Value {
    kind: ValueKind::Ident("x".to_string()),
    extra: ValueExtra::new(Some(Type::Int), None),
    tag: None,
  };

  program.add_statement(Statement::Invoke {
    kind: InvokeKind::Static,
    callee: Value {
      kind: ValueKind::Ident("foo".to_string()),
      extra: ValueExtra::new(Some(Type::Int), None),
      tag: None,
    },
    args: vec![Value {
      kind: ValueKind::Exp(Expr::BinOp {
        left: Box::new(Value {
          kind: ValueKind::Exp(Expr::New { t: Type::Int }),
          extra: ValueExtra::new(Some(Type::Int), None),
          tag: None,
        }),
        right: Box::new(Value {
          kind: ValueKind::Exp(Expr::New { t: Type::Int }),
          extra: ValueExtra::new(Some(Type::Int), None),
          tag: None,
        }),
        op: BinaryOp::Add,
      }),
      extra: ValueExtra::new(Some(Type::Int), None),
      tag: None,
    }],
    ret_type: Type::Int,
    ret_loc: Some(x),
    tag: None,
  });

  let expected = "0: x: int = static foo: int((new int + new int)) : int";
  assert_eq!(PrettyPrinter::print_program(&program), expected);
}

#[test]
fn test_define_statements() {
  let mut program = Program::new(PathBuf::new());

  // Test class definition
  program.add_statement(Statement::Define {
    kind: DefineKind::Class {
      name: "Test".to_string(),
      super_class: Some("Base".to_string()),
      interfaces: vec!["I1".to_string(), "I2".to_string()],
      body: vec![],
    },
    scope_id: None,
    tag: None,
  });

  // Test method definition
  program.add_statement(Statement::Define {
    kind: DefineKind::Method {
      sig: MethodSig {
        name: "test".to_string(),
        params: vec![
          (Type::Int, Some("x".to_string())),
          (Type::String, Some("y".to_string())),
        ],
      },
      body: vec![],
      ret_type: vec![Type::Int],
    },
    scope_id: None,
    tag: None,
  });

  let expected = r#"0: define class Test extends Base implements I1, I2
1: define method test(x: int, y: string): int"#;
  assert_eq!(PrettyPrinter::print_program(&program), expected);
}

#[test]
fn test_all_types() {
  let mut program = Program::new(PathBuf::new());

  // Test all type variants
  let types = vec![
    (Type::Int, "int"),
    (Type::Float, "float"),
    (Type::Double, "double"),
    (Type::Long, "long"),
    (
      Type::Named {
        name: "String".to_string(),
        generics: vec![],
      },
      "String",
    ),
    (Type::Null, "null"),
    (Type::Array(Box::new(Type::Int), Some(10)), "int[10]"),
    (Type::Array(Box::new(Type::Int), None), "int[?]"),
    (Type::Bool, "boolean"),
  ];

  let mut expected_lines = Vec::new();
  for (i, (t, _)) in types.iter().enumerate() {
    let var = Value {
      kind: ValueKind::Ident("x".to_string()),
      extra: ValueExtra::new(Some(t.clone()), None),
      tag: None,
    };
    program.add_statement(Statement::Assign {
      left: var.clone(),
      right: Value {
        kind: ValueKind::NullLit,
        extra: ValueExtra::new(None, None),
        tag: None,
      },
      tag: None,
    });
    expected_lines.push(format!(
      "{}: {} = null",
      i,
      PrettyPrinter::print_value(&var)
    ));
  }
  let expected = expected_lines.join("\n");
  assert_eq!(PrettyPrinter::print_program(&program), expected);
}

#[test]
fn test_all_binary_ops() {
  let mut program = Program::new(PathBuf::new());

  // Test all binary operations
  let ops = vec![
    (BinaryOp::Add, "+"),
    (BinaryOp::Sub, "-"),
    (BinaryOp::Mul, "*"),
    (BinaryOp::Div, "/"),
    (BinaryOp::And, "&"),
    (BinaryOp::Or, "|"),
    (BinaryOp::Xor, "^"),
    (BinaryOp::Shl, "<<"),
    (BinaryOp::Shr, ">>"),
    (BinaryOp::Ushr, ">>>"),
    (BinaryOp::Eq, "=="),
    (BinaryOp::Neq, "!="),
    (BinaryOp::Lt, "<"),
    (BinaryOp::Lte, "<="),
    (BinaryOp::Gt, ">"),
    (BinaryOp::Gte, ">="),
  ];

  for (op, _) in ops {
    let x = Value {
      kind: ValueKind::Ident("x".to_string()),
      extra: ValueExtra::new(Some(Type::Int), None),
      tag: None,
    };
    program.add_statement(Statement::Assign {
      left: x,
      right: Value {
        kind: ValueKind::Exp(Expr::BinOp {
          left: Box::new(Value {
            kind: ValueKind::Exp(Expr::New { t: Type::Int }),
            extra: ValueExtra::new(Some(Type::Int), None),
            tag: None,
          }),
          right: Box::new(Value {
            kind: ValueKind::Exp(Expr::New { t: Type::Int }),
            extra: ValueExtra::new(Some(Type::Int), None),
            tag: None,
          }),
          op: op.clone(),
        }),
        extra: ValueExtra::new(Some(Type::Int), None),
        tag: None,
      },
      tag: None,
    });
  }

  let expected = r#"0: x: int = (new int + new int)
1: x: int = (new int - new int)
2: x: int = (new int * new int)
3: x: int = (new int / new int)
4: x: int = (new int & new int)
5: x: int = (new int | new int)
6: x: int = (new int ^ new int)
7: x: int = (new int << new int)
8: x: int = (new int >> new int)
9: x: int = (new int >>> new int)
10: x: int = (new int == new int)
11: x: int = (new int != new int)
12: x: int = (new int < new int)
13: x: int = (new int <= new int)
14: x: int = (new int > new int)
15: x: int = (new int >= new int)"#;
  assert_eq!(PrettyPrinter::print_program(&program), expected);
}

#[test]
fn test_unary_ops() {
  let mut program = Program::new(PathBuf::new());

  // Test unary operations
  let ops = vec![(UnaryOp::Neg, "-"), (UnaryOp::Not, "!")];

  for (op, _) in ops {
    let x = Value {
      kind: ValueKind::Ident("x".to_string()),
      extra: ValueExtra::new(Some(Type::Int), None),
      tag: None,
    };
    program.add_statement(Statement::Assign {
      left: x,
      right: Value {
        kind: ValueKind::Exp(Expr::UnOp {
          value: Box::new(Value {
            kind: ValueKind::Exp(Expr::New { t: Type::Int }),
            extra: ValueExtra::new(Some(Type::Int), None),
            tag: None,
          }),
          op: op.clone(),
        }),
        extra: ValueExtra::new(Some(Type::Int), None),
        tag: None,
      },
      tag: None,
    });
  }

  let expected = r#"0: x: int = -new int
1: x: int = !new int"#;
  assert_eq!(PrettyPrinter::print_program(&program), expected);
}

#[test]
fn test_all_value_types() {
  let mut program = Program::new(PathBuf::new());

  // Test all value variants
  let values = vec![
    (
      Value {
        kind: ValueKind::NullLit,
        extra: ValueExtra::new(None, None),
        tag: None,
      },
      "null",
    ),
    (
      Value {
        kind: ValueKind::IntLit(42),
        extra: ValueExtra::new(Some(Type::Int), None),
        tag: None,
      },
      "42",
    ),
    (
      Value {
        kind: ValueKind::LongLit(42),
        extra: ValueExtra::new(Some(Type::Long), None),
        tag: None,
      },
      "42L",
    ),
    (
      Value {
        kind: ValueKind::FloatLit(42.0),
        extra: ValueExtra::new(Some(Type::Float), None),
        tag: None,
      },
      "42f",
    ),
    (
      Value {
        kind: ValueKind::DoubleLit(42.0),
        extra: ValueExtra::new(Some(Type::Double), None),
        tag: None,
      },
      "42",
    ),
    (
      Value {
        kind: ValueKind::StringLit("test".to_string()),
        extra: ValueExtra::new(
          Some(Type::Named {
            name: "String".to_string(),
            generics: vec![],
          }),
          None,
        ),
        tag: None,
      },
      "\"test\"",
    ),
    (
      Value {
        kind: ValueKind::TypeLit(Type::Named {
          name: "Test".to_string(),
          generics: vec![],
        }),
        extra: ValueExtra::new(None, None),
        tag: None,
      },
      "Test.type",
    ),
  ];

  for (v, _) in values {
    let x = Value {
      kind: ValueKind::Ident("x".to_string()),
      extra: ValueExtra::new(Some(Type::Int), None),
      tag: None,
    };
    program.add_statement(Statement::Assign {
      left: x,
      right: v.clone(),
      tag: None,
    });
  }

  let expected = r#"0: x: int = null
1: x: int = 42
2: x: int = 42L
3: x: int = 42f
4: x: int = 42
5: x: int = "test"
6: x: int = Test.type"#;
  assert_eq!(PrettyPrinter::print_program(&program), expected);
}

#[test]
fn test_all_invoke_kinds() {
  let mut program = Program::new(PathBuf::new());

  // Test all invoke kinds
  let base = Value {
    kind: ValueKind::Ident("obj".to_string()),
    extra: ValueExtra::new(
      Some(Type::Named {
        name: "Test".to_string(),
        generics: vec![],
      }),
      None,
    ),
    tag: None,
  };

  let kinds = vec![
    (InvokeKind::Static, "static"),
    (
      InvokeKind::Virtual {
        base: Box::new(base.clone()),
      },
      "virtual obj: Test",
    ),
    (
      InvokeKind::Interface {
        base: Box::new(base.clone()),
      },
      "interface obj: Test",
    ),
  ];

  for (kind, _) in kinds {
    let x = Value {
      kind: ValueKind::Ident("x".to_string()),
      extra: ValueExtra::new(Some(Type::Int), None),
      tag: None,
    };
    program.add_statement(Statement::Invoke {
      kind: kind.clone(),
      callee: Value {
        kind: ValueKind::Ident("test".to_string()),
        extra: ValueExtra::new(Some(Type::Int), None),
        tag: None,
      },
      args: vec![Value {
        kind: ValueKind::IntLit(42),
        extra: ValueExtra::new(Some(Type::Int), None),
        tag: None,
      }],
      ret_type: Type::Int,
      ret_loc: Some(x),
      tag: None,
    });
  }

  let expected = r#"0: x: int = static test: int(42) : int
1: x: int = virtual obj: Test test: int(42) : int
2: x: int = interface obj: Test test: int(42) : int"#;
  assert_eq!(PrettyPrinter::print_program(&program), expected);
}

#[test]
fn test_all_define_kinds() {
  let mut program = Program::new(PathBuf::new());

  // Test all define kinds
  program.add_statement(Statement::Define {
    kind: DefineKind::Class {
      name: "Test".to_string(),
      super_class: Some("Base".to_string()),
      interfaces: vec!["I1".to_string()],
      body: vec![],
    },
    scope_id: None,
    tag: None,
  });

  program.add_statement(Statement::Define {
    kind: DefineKind::Interface {
      name: "I1".to_string(),
      extends: Some("I0".to_string()),
    },
    scope_id: None,
    tag: None,
  });

  program.add_statement(Statement::Define {
    kind: DefineKind::Method {
      sig: MethodSig {
        name: "test".to_string(),
        params: vec![(Type::Int, Some("x".to_string()))],
      },
      body: vec![],
      ret_type: vec![Type::Int],
    },
    scope_id: None,
    tag: None,
  });

  program.add_statement(Statement::Define {
    kind: DefineKind::Field {
      name: "field".to_string(),
      init: Some(Value {
        kind: ValueKind::IntLit(42),
        extra: ValueExtra::new(Some(Type::Int), None),
        tag: None,
      }),
      t: Some(Type::Int),
    },
    scope_id: None,
    tag: None,
  });

  program.add_statement(Statement::Define {
    kind: DefineKind::Constructor {
      sig: MethodSig {
        name: "Test".to_string(),
        params: vec![(Type::Int, Some("x".to_string()))],
      },
      body: vec![],
    },
    scope_id: None,
    tag: None,
  });
  let expected = r#"0: define class Test extends Base implements I1
1: define interface I1 extends I0
2: define method test(x: int): int
3: define field field: int = 42
4: define constructor Test(x: int)"#;
  assert_eq!(PrettyPrinter::print_program(&program), expected);
}

#[test]
fn test_nested_if_statements() {
  let mut program = Program::new(PathBuf::new());

  // Create a program with nested if statements:
  // if x then {
  //   if y then {
  //     return 1
  //   } else {
  //     return 2
  //   }
  // } else {
  //   return 3
  // }

  let x = Value {
    kind: ValueKind::Ident("x".to_string()),
    extra: ValueExtra::new(Some(Type::Int), None),
    tag: None,
  };
  let y = Value {
    kind: ValueKind::Ident("y".to_string()),
    extra: ValueExtra::new(Some(Type::Int), None),
    tag: None,
  };

  program.add_statement(Statement::If {
    condition: x,
    then_stmts: vec![Statement::If {
      condition: y,
      then_stmts: vec![Statement::Return {
        value: Some(Value {
          kind: ValueKind::IntLit(1),
          extra: ValueExtra::new(Some(Type::Int), None),
          tag: None,
        }),
        tag: None,
      }],
      else_stmts: vec![Statement::Return {
        value: Some(Value {
          kind: ValueKind::IntLit(2),
          extra: ValueExtra::new(Some(Type::Int), None),
          tag: None,
        }),
        tag: None,
      }],
      tag: None,
    }],
    else_stmts: vec![Statement::Return {
      value: Some(Value {
        kind: ValueKind::IntLit(3),
        extra: ValueExtra::new(Some(Type::Int), None),
        tag: None,
      }),
      tag: None,
    }],
    tag: None,
  });

  let expected = r#"0: if x: int then {
  0: if y: int then {
    0: return 1
  } else {
    0: return 2
  }
} else {
  0: return 3
}"#;

  assert_eq!(PrettyPrinter::print_program(&program), expected);
}

#[test]
fn test_deref_and_dot_access() {
  let mut program = Program::new(PathBuf::new());

  // Test dereferencing and field access:
  // x = *ptr
  // y = obj.field
  // z = *obj.field

  let x = Value {
    kind: ValueKind::Ident("x".to_string()),
    extra: ValueExtra::new(Some(Type::Int), None),
    tag: None,
  };
  let y = Value {
    kind: ValueKind::Ident("y".to_string()),
    extra: ValueExtra::new(Some(Type::Int), None),
    tag: None,
  };
  let z = Value {
    kind: ValueKind::Ident("z".to_string()),
    extra: ValueExtra::new(Some(Type::Int), None),
    tag: None,
  };
  let ptr = Value {
    kind: ValueKind::Ident("ptr".to_string()),
    extra: ValueExtra::new(
      Some(Type::Named {
        name: "int*".to_string(),
        generics: vec![],
      }),
      None,
    ),
    tag: None,
  };
  let obj = Value {
    kind: ValueKind::Ident("obj".to_string()),
    extra: ValueExtra::new(
      Some(Type::Named {
        name: "Test".to_string(),
        generics: vec![],
      }),
      None,
    ),
    tag: None,
  };

  // x = *ptr
  program.add_statement(Statement::Assign {
    left: x.clone(),
    right: Value {
      kind: ValueKind::Exp(Expr::Deref {
        value: Box::new(ptr.clone()),
      }),
      extra: ValueExtra::new(Some(Type::Int), None),
      tag: None,
    },
    tag: None,
  });

  // y = obj.field
  program.add_statement(Statement::Assign {
    left: y.clone(),
    right: Value {
      kind: ValueKind::Exp(Expr::DotAccess {
        base: Box::new(obj.clone()),
        field: "field".to_string(),
      }),
      extra: ValueExtra::new(Some(Type::Int), None),
      tag: None,
    },
    tag: None,
  });

  // z = *obj.field
  program.add_statement(Statement::Assign {
    left: z,
    right: Value {
      kind: ValueKind::Exp(Expr::Deref {
        value: Box::new(Value {
          kind: ValueKind::Exp(Expr::DotAccess {
            base: Box::new(obj),
            field: "field".to_string(),
          }),
          extra: ValueExtra::new(Some(Type::Int), None),
          tag: None,
        }),
      }),
      extra: ValueExtra::new(Some(Type::Int), None),
      tag: None,
    },
    tag: None,
  });

  let expected = r#"0: x: int = *ptr: int*
1: y: int = (obj: Test).field
2: z: int = *(obj: Test).field"#;
  assert_eq!(PrettyPrinter::print_program(&program), expected);
}
