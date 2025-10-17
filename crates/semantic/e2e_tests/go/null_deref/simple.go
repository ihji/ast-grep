package main

func main() {
	// CREATE[null]: test1
	var p *int
	// FIND[NULL_DEREFERENCE]: test1
	x := *p
	println(x)
}

func foo(x *int) {
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
