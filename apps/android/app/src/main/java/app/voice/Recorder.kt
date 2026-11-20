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

    /** Why recording stopped by itself (the microphone failed), or null. */
    @Volatile var error: String? = null
        private set

    /** Set when the last stop() could not end the reader thread in time (null when it did). */
    @Volatile var stopProblem: String? = null
        private set

    /** False when the microphone could not be opened (no permission, or in use). */
    @SuppressLint("MissingPermission") // checked by the caller
    fun start(): Boolean {
        error = null
        val min = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        if (min <= 0) return false
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
        // startRecording() can also fail (microphone taken by a call or another app).
        try {
            r.startRecording()
        } catch (e: Exception) {
            r.release()
            return false
        }
        if (r.recordingState != AudioRecord.RECORDSTATE_RECORDING) {
            r.release()
            return false
        }
        synchronized(pcm) { pcm.reset() }
        record = r
        running = true
        thread = Thread {
            val chunk = ByteArray(RATE / 10 * 2)
            while (running) {
                val n = try {
                    r.read(chunk, 0, chunk.size)
                } catch (e: Exception) {
                    AudioRecord.ERROR
                }
                when {
                    n > 0 -> {
                        synchronized(pcm) { pcm.write(chunk, 0, n) }
                        level = rms(chunk, n)
                    }
                    n == 0 -> try {
                        Thread.sleep(5)
                    } catch (_: InterruptedException) {
                        running = false
                    }
                    // Negative codes (ERROR, ERROR_INVALID_OPERATION, ERROR_BAD_VALUE,
                    // ERROR_DEAD_OBJECT): stop instead of spinning at full processor load.
                    else -> {
                        if (running) error = "The microphone stopped working (code $n)."
                        running = false
                    }
                }
            }
        }.also { it.start() }
        return true
    }

    /**
     * Stops and returns the recording as 16-bit PCM bytes. The reading thread has finished
     * before the bytes are copied and the microphone is released.
     */
    fun stop(): ByteArray {
        running = false
        val r = record
        // stop() unblocks a read() that is waiting for audio.
        try {
            r?.stop()
        } catch (_: Exception) {
        }
        val t = thread
        if (t != null) {
            t.join(2000)
            if (t.isAlive) {
                t.interrupt()
                t.join(500)
            }
        }
        thread = null
        record = null
        level = 0f
        if (t != null && t.isAlive) {
            // The reader is still inside read(): releasing the microphone now could crash it.
            // Release it once the reader has really ended, and tell the caller.
            stopProblem = "The microphone did not stop in time; it is released in the background."
            Thread {
                t.join(10_000)
                if (!t.isAlive) {
                    try {
                        r?.release()
                    } catch (_: Exception) {
                    }
                }
            }.apply { isDaemon = true }.start()
        } else {
            stopProblem = null
            r?.release()
        }
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
