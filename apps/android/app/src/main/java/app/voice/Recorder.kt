package app.voice

import android.annotation.SuppressLint
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.sqrt

/** Records 16 kHz mono 16-bit audio in memory. Nothing is written to storage. */
class Recorder {
    private var record: AudioRecord? = null
    private var thread: Thread? = null
    private val pcm = ByteArrayOutputStream()

    @Volatile private var running = false

    /** Loudness of the last 100 ms, 0..1, for the level bar. */
    @Volatile var level = 0f
        private set

    /** False when the microphone could not be opened (no permission, or in use). */
    @SuppressLint("MissingPermission") // checked by the caller
    fun start(): Boolean {
        val min = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val r = try {
            AudioRecord(
                MediaRecorder.AudioSource.VOICE_RECOGNITION,
                RATE,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                maxOf(min, RATE * 2),
            )
        } catch (e: Exception) {
            return false
        }
        if (r.state != AudioRecord.STATE_INITIALIZED) {
            r.release()
            return false
        }
        synchronized(pcm) { pcm.reset() }
        r.startRecording()
        record = r
        running = true
        thread = Thread {
            val chunk = ByteArray(RATE / 10 * 2)
            while (running) {
                val n = r.read(chunk, 0, chunk.size)
                if (n > 0) {
                    synchronized(pcm) { pcm.write(chunk, 0, n) }
                    level = rms(chunk, n)
                }
            }
        }.also { it.start() }
        return true
    }

    /** Stops and returns the recording as 16-bit PCM bytes. */
    fun stop(): ByteArray {
        running = false
        thread?.join(1000)
        thread = null
        record?.let {
            try {
                it.stop()
            } catch (_: Exception) {
            }
            it.release()
        }
        record = null
        level = 0f
        return synchronized(pcm) { pcm.toByteArray() }
    }

    companion object {
        const val RATE = 16_000

        fun rms(bytes: ByteArray, n: Int): Float {
            val b = ByteBuffer.wrap(bytes, 0, n).order(ByteOrder.LITTLE_ENDIAN).asShortBuffer()
            var sum = 0.0
            val count = b.remaining()
            while (b.hasRemaining()) {
                val s = b.get() / 32768.0
                sum += s * s
            }
            return if (count == 0) 0f else sqrt(sum / count).toFloat()
        }

        fun durationMs(pcm: ByteArray) = pcm.size / 2 * 1000L / RATE

        /** A WAV file around raw 16 kHz mono PCM. */
        fun wav(pcm: ByteArray): ByteArray {
            val h = ByteBuffer.allocate(44).order(ByteOrder.LITTLE_ENDIAN)
            h.put("RIFF".toByteArray()).putInt(36 + pcm.size).put("WAVE".toByteArray())
            h.put("fmt ".toByteArray()).putInt(16).putShort(1.toShort()).putShort(1.toShort())
            h.putInt(RATE).putInt(RATE * 2).putShort(2.toShort()).putShort(16.toShort())
            h.put("data".toByteArray()).putInt(pcm.size)
            return h.array() + pcm
        }
    }
}
