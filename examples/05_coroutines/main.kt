// Example 05: Suspend Functions & Coroutines

package examples.coroutines

suspend fun fetchCompilerStatus(): String {
    // Represents an asynchronous task lowered to CPS state machine
    return "Kotlin3 compiler is ready."
}

suspend fun computeBuildTime(): Long {
    return 15L // 15ms target
}

fun main() {
    println("Coroutines transform suspend functions into state-machines.")
    println("Kotlin3 lowers these in k3_ir via AddContinuationLowering.")
}
