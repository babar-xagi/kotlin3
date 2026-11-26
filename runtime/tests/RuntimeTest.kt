package k3.runtime

import k3.test.expectEquals
import k3.test.expectThrows
import k3.test.expectTrue
import k3.test.runTests
import k3.test.test

fun main() {
    runTests("k3-runtime") {
        test("K3Env returns default for unset variable") {
            expectEquals(42, K3Env.int("K3_NONEXISTENT_TEST_VAR_XYZ", 42))
        }

        test("K3Json roundtrip object and array") {
            val payload = mapOf(
                "name" to "kotlin3",
                "version" to 7L,
                "fast" to true,
                "tags" to listOf("rust", "kotlin")
            )
            val json = K3Json.stringify(payload)
            @Suppress("UNCHECKED_CAST")
            val parsed = K3Json.parse(json) as Map<String, Any?>
            expectEquals("kotlin3", parsed["name"])
            expectEquals(7L, parsed["version"])
            expectEquals(true, parsed["fast"])
            expectEquals(listOf("rust", "kotlin"), parsed["tags"])
        }

        test("K3Time measure captures elapsed ms and value") {
            val res = K3Time.measure {
                var sum = 0L
                for (i in 1..1000) sum += i
                sum
            }
            expectEquals(500500L, res.value)
            expectTrue(res.elapsedMs >= 0.0)
        }

        test("K3Async parallel computes results") {
            val out = K3Async.parallel(listOf({ 10 + 1 }, { 20 + 2 }, { 30 + 3 }))
            expectEquals(listOf(11, 22, 33), out)
        }

        test("expectThrows catches errors") {
            expectThrows {
                error("boom")
            }
        }
    }
}
