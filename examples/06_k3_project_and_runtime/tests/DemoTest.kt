package demo

import k3.test.expectEquals
import k3.test.runTests
import k3.test.test

fun main() {
    runTests("DemoProjectTest") {
        test("summarizeBuild serializes squared list to JSON") {
            val out = summarizeBuild("k3-demo", listOf(2, 3))
            expectEquals("""{"project":"k3-demo","count":2,"squares":[4,9]}""", out)
        }
    }
}
