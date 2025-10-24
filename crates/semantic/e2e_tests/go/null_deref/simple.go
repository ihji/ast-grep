package main

func foo() {
	// CREATE[null]: test1
	var p *int
	// FIND[NULL_DEREFERENCE]: test1
	x := *p
	println(x)
}

func bar(x *int) {
	// ASSUME[true]: test2
	if x == nil {
		println(x)
	}
	// ASSUME[false]: test2
	if x != nil {
		println(x)
	}
	// FIND[NULL_DEREFERENCE]: test2
	y := *x
	println(y)
}

func baz() {
	// CREATE[null]: test3
	var p *int
	// FIND[NULL_DEREFERENCE]: test3
	*p = 10
	println(p)
}
