package main

type Node struct {
	Value int
	Next  *Node
}

func directFieldOnNilPointer() {
	// CREATE[null]: test
	var n *Node
	// FIND[NULL_DEREFERENCE]: test
	_ = n.Value // panic: invalid memory address or nil pointer dereference
}

func nestedFieldInnerNil() {
	// TODO_CREATE: non-nil outer *Node but inner Next is nil
	n := &Node{Value: 42, Next: nil}
	// TODO_FIND: NULL_DEREFERENCE (nested field access; n.Next is nil)
	_ = n.Next.Value // panic: invalid memory address or nil pointer dereference
}
