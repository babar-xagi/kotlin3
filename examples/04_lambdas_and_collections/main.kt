// Example 04: Higher-Order Functions, Lambdas & Collections

package examples.lambdas

fun <T> List<T>.customFilter(predicate: (T) -> Boolean): List<T> {
    val result = mutableListOf<T>()
    for (item in this) {
        if (predicate(item)) {
            result.add(item)
        }
    }
    return result
}

fun main() {
    val numbers = listOf(1, 2, 3, 4, 5, 6, 7, 8, 9, 10)

    val evens = numbers.customFilter { it % 2 == 0 }
    val doubled = evens.map { it * 2 }

    println("Original: $numbers")
    println("Evens:    $evens")
    println("Doubled:  $doubled")
}
