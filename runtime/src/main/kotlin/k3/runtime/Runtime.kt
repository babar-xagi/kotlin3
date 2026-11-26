package k3.runtime

import java.io.File
import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.time.Duration
import java.util.Locale
import java.util.concurrent.Callable
import java.util.concurrent.Executors

data class TimedResult<T>(val value: T, val elapsedMs: Double)

data class CommandOutput(val exitCode: Int, val stdout: String, val stderr: String)

data class HttpResponseData(val status: Int, val body: String)

object K3Env {
    fun get(name: String, default: String = ""): String =
        System.getenv(name) ?: default

    fun int(name: String, default: Int): Int {
        val raw = System.getenv(name) ?: return default
        return raw.toIntOrNull()
            ?: error("Environment variable '$name' must be an integer, got '$raw'")
    }

    fun requirePositiveInt(name: String, default: Int): Int {
        val value = int(name, default)
        require(value > 0) { "$name must be > 0 (got $value)" }
        return value
    }
}

object K3Time {
    fun <T> measure(block: () -> T): TimedResult<T> {
        val start = System.nanoTime()
        val result = block()
        val elapsed = (System.nanoTime() - start) / 1_000_000.0
        return TimedResult(result, elapsed)
    }

    fun formatMs(ms: Double): String = "%.3f".format(Locale.US, ms)
}

object K3Fs {
    fun readText(path: String): String =
        Files.readString(Path.of(path), StandardCharsets.UTF_8)

    fun writeText(path: String, content: String) {
        val p = Path.of(path)
        p.parent?.let { Files.createDirectories(it) }
        Files.writeString(p, content, StandardCharsets.UTF_8)
    }

    fun exists(path: String): Boolean = Files.exists(Path.of(path))

    fun listFiles(dir: String): List<String> =
        File(dir).listFiles()?.map { it.path }?.sorted() ?: emptyList()
}

object K3Process {
    fun exec(vararg command: String, workDir: String? = null): CommandOutput {
        val pb = ProcessBuilder(*command)
        if (workDir != null) {
            pb.directory(File(workDir))
        }
        val proc = pb.start()
        val stdout = proc.inputStream.bufferedReader(StandardCharsets.UTF_8).readText()
        val stderr = proc.errorStream.bufferedReader(StandardCharsets.UTF_8).readText()
        val code = proc.waitFor()
        return CommandOutput(code, stdout, stderr)
    }
}

object K3Http {
    private val client: HttpClient by lazy {
        HttpClient.newBuilder()
            .connectTimeout(Duration.ofSeconds(10))
            .followRedirects(HttpClient.Redirect.NORMAL)
            .build()
    }

    fun get(url: String): HttpResponseData {
        val req = HttpRequest.newBuilder(URI.create(url))
            .timeout(Duration.ofSeconds(20))
            .GET()
            .build()
        val resp = client.send(req, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8))
        return HttpResponseData(resp.statusCode(), resp.body())
    }

    fun post(url: String, body: String, contentType: String = "application/json"): HttpResponseData {
        val req = HttpRequest.newBuilder(URI.create(url))
            .timeout(Duration.ofSeconds(20))
            .header("Content-Type", contentType)
            .POST(HttpRequest.BodyPublishers.ofString(body, StandardCharsets.UTF_8))
            .build()
        val resp = client.send(req, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8))
        return HttpResponseData(resp.statusCode(), resp.body())
    }
}

object K3Async {
    fun <T> parallel(tasks: List<() -> T>): List<T> {
        val pool = Executors.newVirtualThreadPerTaskExecutor()
        return pool.use { exec ->
            val futures = tasks.map { fn -> exec.submit(Callable { fn() }) }
            futures.map { it.get() }
        }
    }
}

object K3Json {
    fun stringify(value: Any?): String = when (value) {
        null -> "null"
        is Boolean, is Number -> value.toString()
        is String -> buildString {
            append('"')
            for (ch in value) {
                when (ch) {
                    '"' -> append("\\\"")
                    '\\' -> append("\\\\")
                    '\n' -> append("\\n")
                    '\r' -> append("\\r")
                    '\t' -> append("\\t")
                    else -> append(ch)
                }
            }
            append('"')
        }
        is Map<*, *> -> value.entries.joinToString(separator = ",", prefix = "{", postfix = "}") { (k, v) ->
            "${stringify(k.toString())}:${stringify(v)}"
        }
        is Iterable<*> -> value.joinToString(separator = ",", prefix = "[", postfix = "]") { stringify(it) }
        else -> stringify(value.toString())
    }

    fun parse(input: String): Any? = JsonParser(input).parseValue()

    private class JsonParser(private val src: String) {
        private var pos = 0

        fun parseValue(): Any? {
            skipWs()
            if (pos >= src.length) error("Unexpected end of JSON")
            return when (src[pos]) {
                '{' -> parseObject()
                '[' -> parseArray()
                '"' -> parseString()
                't' -> expectLiteral("true", true)
                'f' -> expectLiteral("false", false)
                'n' -> expectLiteral("null", null)
                '-', in '0'..'9' -> parseNumber()
                else -> error("Unexpected JSON token '${src[pos]}' at index $pos")
            }
        }

        private fun parseObject(): Map<String, Any?> {
            pos++ // '{'
            skipWs()
            val map = LinkedHashMap<String, Any?>()
            if (peek('}')) {
                pos++
                return map
            }
            while (true) {
                skipWs()
                val key = parseString()
                skipWs()
                expect(':')
                val valObj = parseValue()
                map[key] = valObj
                skipWs()
                if (peek(',')) {
                    pos++
                } else if (peek('}')) {
                    pos++
                    return map
                } else {
                    error("Expected ',' or '}' in JSON object at $pos")
                }
            }
        }

        private fun parseArray(): List<Any?> {
            pos++ // '['
            skipWs()
            val list = ArrayList<Any?>()
            if (peek(']')) {
                pos++
                return list
            }
            while (true) {
                list.add(parseValue())
                skipWs()
                if (peek(',')) {
                    pos++
                } else if (peek(']')) {
                    pos++
                    return list
                } else {
                    error("Expected ',' or ']' in JSON array at $pos")
                }
            }
        }

        private fun parseString(): String {
            expect('"')
            val sb = StringBuilder()
            while (pos < src.length) {
                val ch = src[pos++]
                if (ch == '"') return sb.toString()
                if (ch == '\\' && pos < src.length) {
                    when (val esc = src[pos++]) {
                        '"' -> sb.append('"')
                        '\\' -> sb.append('\\')
                        '/' -> sb.append('/')
                        'n' -> sb.append('\n')
                        'r' -> sb.append('\r')
                        't' -> sb.append('\t')
                        'u' -> {
                            val hex = src.substring(pos, pos + 4)
                            pos += 4
                            sb.append(hex.toInt(16).toChar())
                        }
                        else -> sb.append(esc)
                    }
                } else {
                    sb.append(ch)
                }
            }
            error("Unterminated JSON string")
        }

        private fun parseNumber(): Number {
            val start = pos
            if (peek('-')) pos++
            while (pos < src.length && src[pos].isDigit()) pos++
            var isFloat = false
            if (pos < src.length && src[pos] == '.') {
                isFloat = true
                pos++
                while (pos < src.length && src[pos].isDigit()) pos++
            }
            if (pos < src.length && (src[pos] == 'e' || src[pos] == 'E')) {
                isFloat = true
                pos++
                if (pos < src.length && (src[pos] == '+' || src[pos] == '-')) pos++
                while (pos < src.length && src[pos].isDigit()) pos++
            }
            val raw = src.substring(start, pos)
            return if (isFloat) raw.toDouble() else raw.toLong()
        }

        private fun <T> expectLiteral(lit: String, value: T): T {
            if (!src.startsWith(lit, pos)) error("Expected '$lit' at $pos")
            pos += lit.length
            return value
        }

        private fun skipWs() {
            while (pos < src.length && src[pos].isWhitespace()) pos++
        }

        private fun peek(c: Char): Boolean = pos < src.length && src[pos] == c

        private fun expect(c: Char) {
            if (pos >= src.length || src[pos] != c) error("Expected '$c' at $pos")
            pos++
        }
    }
}
