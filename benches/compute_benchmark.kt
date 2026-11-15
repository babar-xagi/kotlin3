private const val ONE: Long = 1L shl 16

private fun envInt(name: String, fallback: Int): Int {
    val raw = System.getenv(name) ?: return fallback
    val value = raw.toIntOrNull() ?: error("$name must be an integer")
    require(value > 0) { "$name must be > 0" }
    return value
}

private fun mix32(value: Int): Int {
    var x = value
    x = x xor (x ushr 16)
    x *= 0x7FEB352D
    x = x xor (x ushr 15)
    x *= 0x846CA68B.toInt()
    x = x xor (x ushr 16)
    return x
}

private data class Pair32(val first: Int, val second: Int)

private fun benchAlu(iterations: Int): Pair32 {
    var x = 0x12345678
    var acc = 0x9E3779B9.toInt()
    for (i in 0 until iterations) {
        x = mix32(x xor i)
        val term = i * 0x045D9F3B
        if ((x and 15) == 0) acc += (x xor term)
        else acc = acc xor (x + term)
    }
    return Pair32(x, acc)
}

private fun benchMemory(iterations: Int, size: Int): Pair32 {
    require(size and (size - 1) == 0) { "BENCH_MEM_SIZE must be a power of two" }
    val values = IntArray(size)
    for (i in values.indices) values[i] = mix32(i xor 0xA5A5A5A5.toInt())
    val mask = size - 1
    var index = 0x12345678 and mask
    var acc = 0xC001D00D.toInt()
    for (i in 0 until iterations) {
        index = (index * 1664525 + 1013904223) and mask
        val value = mix32(values[index] xor i xor index)
        values[index] = value
        if ((value and 3) == 0) acc += (value xor index)
        else acc = acc xor (value + index)
    }
    return Pair32(acc, values[index])
}

private data class SieveResult(val count: Int, val sum: Long)

private fun benchSieve(limit: Int): SieveResult {
    val flags = ByteArray(limit + 1) { 1 }
    flags[0] = 0
    if (limit >= 1) flags[1] = 0
    var p = 2
    while (p.toLong() * p <= limit.toLong()) {
        if (flags[p].toInt() != 0) {
            var multiple = p * p
            while (multiple <= limit) {
                flags[multiple] = 0
                multiple += p
            }
        }
        p++
    }
    var count = 0
    var sum = 0L
    for (value in 2..limit) {
        if (flags[value].toInt() != 0) {
            count++
            sum += value.toLong()
        }
    }
    return SieveResult(count, sum)
}

private fun benchMatrix(n: Int): Long {
    val cells = n * n
    val a = IntArray(cells)
    val b = IntArray(cells)
    val c = IntArray(cells)
    for (i in 0 until n) {
        val row = i * n
        for (j in 0 until n) {
            a[row + j] = (i * 17 + j * 13 + 7) and 15
            b[row + j] = (i * 11 + j * 19 + 3) and 15
        }
    }
    for (i in 0 until n) {
        val row = i * n
        for (k in 0 until n) {
            val aik = a[row + k]
            val brow = k * n
            for (j in 0 until n) c[row + j] += aik * b[brow + j]
        }
    }
    var total = 0L
    for (value in c) total += value.toLong()
    return total
}

private data class FractalResult(val inside: Int, val totalIterations: Long)

private fun benchFractal(width: Int, height: Int, maxIter: Int): FractalResult {
    var inside = 0
    var totalIterations = 0L
    val bailout = 4L * ONE * ONE
    for (py in 0 until height) {
        val cy = (py.toLong() * (2L * ONE)) / (height - 1).toLong() - ONE
        for (px in 0 until width) {
            val cx = (px.toLong() * (3L * ONE)) / (width - 1).toLong() - 2L * ONE
            var zx = 0L
            var zy = 0L
            var iterations = 0
            while (iterations < maxIter) {
                val xx = zx * zx
                val yy = zy * zy
                if (xx + yy > bailout) break
                val nextX = Math.floorDiv(xx - yy, ONE) + cx
                val nextY = Math.floorDiv(2L * zx * zy, ONE) + cy
                zx = nextX
                zy = nextY
                iterations++
            }
            totalIterations += iterations.toLong()
            if (iterations == maxIter) inside++
        }
    }
    return FractalResult(inside, totalIterations)
}

private inline fun measure(name: String, block: () -> String): Double {
    val start = System.nanoTime()
    val checksum = block()
    val elapsedMs = (System.nanoTime() - start) / 1_000_000.0
    val elapsedStr = "%.3f".format(java.util.Locale.US, elapsedMs)
    println("RESULT|$name|$checksum|$elapsedStr")
    return elapsedMs
}

fun main() {
    val aluIters = envInt("BENCH_ALU_ITERS", 2_000_000)
    val memIters = envInt("BENCH_MEM_ITERS", 1_000_000)
    val memSize = envInt("BENCH_MEM_SIZE", 1 shl 20)
    val sieveLimit = envInt("BENCH_SIEVE_LIMIT", 1_000_000)
    val matrixN = envInt("BENCH_MATRIX_N", 128)
    val fractalWidth = envInt("BENCH_FRACTAL_WIDTH", 320)
    val fractalHeight = envInt("BENCH_FRACTAL_HEIGHT", 180)
    val fractalMaxIter = envInt("BENCH_FRACTAL_MAX_ITER", 128)
    println("CONFIG|alu=$aluIters|mem=$memIters|mem_size=$memSize|sieve=$sieveLimit|matrix=$matrixN|fractal=${fractalWidth}x${fractalHeight}x$fractalMaxIter")
    val totalStart = System.nanoTime()
    measure("alu") { benchAlu(aluIters).let { "${it.first.toUInt()}:${it.second.toUInt()}" } }
    measure("memory") { benchMemory(memIters, memSize).let { "${it.first.toUInt()}:${it.second.toUInt()}" } }
    measure("sieve") { benchSieve(sieveLimit).let { "${it.count}:${it.sum}" } }
    measure("matrix") { benchMatrix(matrixN).toString() }
    measure("fractal") { benchFractal(fractalWidth, fractalHeight, fractalMaxIter).let { "${it.inside}:${it.totalIterations}" } }
    val totalMs = (System.nanoTime() - totalStart) / 1_000_000.0
    val totalStr = "%.3f".format(java.util.Locale.US, totalMs)
    println("TOTAL_MS|$totalStr")
}
