package main

type Node struct {
	Value int
	Next  *Node
}

func directFieldOnNilPointer() {
	// CREATE[null]: test1
	var n *Node
	// FIND[NULL_DEREFERENCE]: test1
	_ = n.Value // panic: invalid memory address or nil pointer dereference
}

func nestedFieldInnerNil() {
	// CREATE[null]: test2
	n := &Node{Value: 42, Next: nil}
	// FIND[NULL_DEREFERENCE]: test2
	_ = n.Next.Value // panic: invalid memory address or nil pointer dereference
}
