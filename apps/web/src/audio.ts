// Microphone recording in the browser, converted to 16 kHz mono 16-bit WAV for Whisper.

export interface Recording {
  stop(): Promise<Blob>;
  cancel(): void;
  level(): number;
}

export async function startRecording(): Promise<Recording> {
  const stream = await navigator.mediaDevices.getUserMedia({
    audio: { echoCancellation: true, noiseSuppression: false, autoGainControl: true, channelCount: 1 },
  });
  const ctx = new AudioContext();
  const source = ctx.createMediaStreamSource(stream);
  const analyser = ctx.createAnalyser();
  analyser.fftSize = 512;
  source.connect(analyser);
  const recorder = new MediaRecorder(stream);
  const chunks: BlobPart[] = [];
  recorder.ondataavailable = (e) => e.data.size && chunks.push(e.data);
  recorder.start();
  const buf = new Float32Array(analyser.fftSize);
  const release = () => {
    stream.getTracks().forEach((t) => t.stop());
    void ctx.close();
  };
  return {
    level() {
      analyser.getFloatTimeDomainData(buf);
      let sum = 0;
      for (const v of buf) sum += v * v;
      return Math.sqrt(sum / buf.length);
    },
    cancel() {
      recorder.onstop = null;
      recorder.stop();
      release();
    },
    stop() {
      return new Promise((resolve, reject) => {
        recorder.onstop = async () => {
          release();
          try {
            const encoded = new Blob(chunks, { type: recorder.mimeType });
            resolve(await toWav16k(encoded));
          } catch (e) {
            reject(e);
          }
        };
        recorder.stop();
      });
    },
  };
}

async function toWav16k(blob: Blob): Promise<Blob> {
  const data = await blob.arrayBuffer();
  const decoded = await new AudioContext().decodeAudioData(data);
  const frames = Math.ceil(decoded.duration * 16000);
  const offline = new OfflineAudioContext(1, frames, 16000);
  const src = offline.createBufferSource();
  src.buffer = decoded;
  src.connect(offline.destination);
  src.start();
  const mono = (await offline.startRendering()).getChannelData(0);
  const out = new DataView(new ArrayBuffer(44 + mono.length * 2));
  const w = (o: number, s: string) => [...s].forEach((c, i) => out.setUint8(o + i, c.charCodeAt(0)));
  w(0, "RIFF");
  out.setUint32(4, 36 + mono.length * 2, true);
  w(8, "WAVEfmt ");
  out.setUint32(16, 16, true);
  out.setUint16(20, 1, true);
  out.setUint16(22, 1, true);
  out.setUint32(24, 16000, true);
  out.setUint32(28, 32000, true);
  out.setUint16(32, 2, true);
  out.setUint16(34, 16, true);
  w(36, "data");
  out.setUint32(40, mono.length * 2, true);
  mono.forEach((v, i) => out.setInt16(44 + i * 2, Math.max(-1, Math.min(1, v)) * 32767, true));
  return new Blob([out], { type: "audio/wav" });
}
