// Example 02: Control Flow, When Expressions & Smart Casts

package examples.controlflow

fun describe(obj: Any): String = when (obj) {
    1          -> "One (Integer)"
    "Hello"    -> "Greeting"
    is Long    -> "Long integer of value $obj"
    !is String -> "Not a string"
    else       -> "Unknown object"
}

fun main() {
    println(describe(1))
    println(describe("Hello"))
    println(describe(1000L))
    println(describe(3.14159))

    // Range loops
    print("Counting: ")
    for (i in 1..5) {
        print("$i ")
    }
    println()
}
