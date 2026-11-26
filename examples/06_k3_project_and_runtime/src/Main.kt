package demo

import k3.runtime.K3Async
import k3.runtime.K3Json
import k3.runtime.K3Time

fun summarizeBuild(name: String, items: List<Int>): String {
    val squared = K3Async.parallel(items.map { n -> { n * n } })
    val payload = mapOf(
        "project" to name,
        "count" to items.size.toLong(),
        "squares" to squared
    )
    return K3Json.stringify(payload)
}

fun main() {
    val timed = K3Time.measure {
        summarizeBuild("k3-demo", listOf(1, 2, 3, 4))
    }
    println("Payload: ${timed.value}")
    println("Elapsed: ${K3Time.formatMs(timed.elapsedMs)} ms")
}
