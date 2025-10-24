package main

func foo(x int) {
	var p *int
	if x > 0 {
		ten := 10
		p = &ten
	}
	if x > 0 {
		y := *p
	    println(y)
	}
}

func main() {
	foo(0)
	foo(10)
}
