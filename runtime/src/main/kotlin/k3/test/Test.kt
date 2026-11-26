package k3.test

import java.util.Locale

private data class TestCaseResult(val name: String, val passed: Boolean, val elapsedMs: Double, val error: String? = null)

private object TestContext {
    var activeResults: MutableList<TestCaseResult>? = null
}

fun test(name: String, block: () -> Unit) {
    val start = System.nanoTime()
    var err: String? = null
    val ok = try {
        block()
        true
    } catch (t: Throwable) {
        err = t.message ?: t::class.java.simpleName
        false
    }
    val ms = (System.nanoTime() - start) / 1_000_000.0
    val formatted = "%.2f".format(Locale.US, ms)
    if (ok) {
        println("  [PASS] $name (${formatted} ms)")
    } else {
        println("  [FAIL] $name (${formatted} ms) -> $err")
    }
    TestContext.activeResults?.add(TestCaseResult(name, ok, ms, err))
}

fun runTests(suiteName: String = "TestSuite", block: () -> Unit) {
    println("Running $suiteName...")
    val results = ArrayList<TestCaseResult>()
    val prev = TestContext.activeResults
    TestContext.activeResults = results
    try {
        block()
    } finally {
        TestContext.activeResults = prev
    }

    val passed = results.count { it.passed }
    val failed = results.size - passed
    val totalMs = results.sumOf { it.elapsedMs }
    val formattedTotal = "%.2f".format(Locale.US, totalMs)
    println("Summary ($suiteName): $passed passed, $failed failed (${formattedTotal} ms)")
    if (failed > 0) {
        System.exit(1)
    }
}

fun <T> expectEquals(expected: T, actual: T, message: String = "") {
    if (expected != actual) {
        val prefix = if (message.isNotEmpty()) "$message: " else ""
        throw AssertionError("${prefix}expected <$expected> but was <$actual>")
    }
}

fun expectTrue(condition: Boolean, message: String = "Expected condition to be true") {
    if (!condition) {
        throw AssertionError(message)
    }
}

fun expectThrows(block: () -> Unit) {
    var threw = false
    try {
        block()
    } catch (_: Throwable) {
        threw = true
    }
    if (!threw) {
        throw AssertionError("Expected block to throw an exception, but it succeeded")
    }
}
