package main

func foo(x int, p **int) {
	if x > 0 {
		*p = nil
	}
}

func main() {
	x := 10
	p := &x
	foo(10, &p)
	// FIND[NULL_DEREFERENCE]: test
	println(*p)
}
