// Example 03: Data Classes, Interfaces & Inheritance

package examples.oop

interface Printable {
    fun printDetails()
}

data class Developer(
    val name: String,
    val role: String,
    val primaryLanguage: String = "Kotlin"
) : Printable {
    override fun printDetails() {
        println("Developer: $name | Role: $role | Primary Language: $primaryLanguage")
    }
}

fun main() {
    val dev1 = Developer("Alice", "Compiler Engineer", "Rust")
    val dev2 = Developer("Bob", "Runtime Architect", "Zig")

    val team = listOf(dev1, dev2)
    for (member in team) {
        member.printDetails()
    }
}
