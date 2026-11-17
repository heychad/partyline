use partyline_recording_check::{Track, RATE, FRAME, recover_wav};
#[test]
fn three_hours_survive_transcription_blackout_and_scheduling_stalls() {
    let dir=tempfile::tempdir().unwrap();
    let paths=[dir.path().join("mic.wav"),dir.path().join("mac.wav")];
    let mut tracks=[Track::create(&paths[0]).unwrap(),Track::create(&paths[1]).unwrap()];
    let data:Vec<f32>=(0..FRAME).map(|n|if n%2==0{0.25}else{-0.25}).collect();
    let mut sent=[0u64;2];
    // Three hours of real 48 kHz PCM is written at accelerated speed. The first
    // channel's network is unavailable throughout; the other reconnects at 2 h.
    // Bursts stand for delayed scheduling, with no timer needed to drain them.
    for frame in 0..108_000 {
        for i in 0..2 {tracks[i].push(&data,|f| {
            assert_eq!(f.start,frame as f64/10.0);sent[i]+=1;i==1 && frame>=72_000
        }).unwrap();}
        if frame%600==0 {for t in &mut tracks{t.checkpoint().unwrap();}}
    }
    assert_eq!(tracks[0].skipped_samples,10800*RATE as u64);
    assert_eq!(tracks[1].skipped_samples,7200*RATE as u64);
    for t in tracks {assert_eq!(t.samples,10800*RATE as u64);t.finish(|_|false).unwrap();}
    assert_eq!(sent,[108_000,108_000]);
    for path in paths {
        let mut r=hound::WavReader::open(path).unwrap();assert_eq!(r.duration() as u64,10800*RATE as u64);
        r.seek(r.duration()-FRAME as u32).unwrap();assert!(r.samples::<i16>().all(|s|s.unwrap().abs()==8192));
    }
}
#[test]
fn checkpoints_tail_silence_and_crash_header_recovery() {
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("mic.wav");
    let mut t=Track::create(&path).unwrap();t.push(&[1.0,-1.0,f32::NAN],|_|false).unwrap();t.checkpoint().unwrap();
    assert_eq!(hound::WavReader::open(&path).unwrap().duration(),3);
    t.silence_until(1.0,|_|false).unwrap();t.finish(|_|false).unwrap();
    {use std::io::{Seek,SeekFrom,Write};let mut f=std::fs::OpenOptions::new().write(true).open(&path).unwrap();f.seek(SeekFrom::Start(40)).unwrap();f.write_all(&6u32.to_le_bytes()).unwrap();}
    assert_eq!(recover_wav(&path).unwrap(),1.0);assert!(path.with_extension("wav.before-recovery").exists());
    let samples:Vec<_>=hound::WavReader::open(&path).unwrap().samples::<i16>().map(Result::unwrap).collect();
    assert_eq!(&samples[..3],&[32767,-32767,0]);assert!(samples[3..].iter().all(|v|*v==0));assert_eq!(recover_wav(&path).unwrap(),1.0);
}
#[test]
fn audio_write_errors_are_not_reported_as_success() {
    let d=tempfile::tempdir().unwrap();assert!(Track::create(d.path()).is_err());
}
