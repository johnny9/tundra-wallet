"use strict";
/* Native, permission-gated QR input. No image upload or external dependency.
 * Unsupported browsers get file/paste alternatives, never a fake scan result.
 */
class TundraDescriptorCamera {
  constructor(){this.stream=null;this.timer=null;this.generation=0;}
  stop(){this.generation++;clearTimeout(this.timer);this.timer=null;this.stream?.getTracks().forEach(track=>track.stop());this.stream=null;}
  async detector(){
    if(!('BarcodeDetector' in globalThis))throw new Error('This browser does not provide QR decoding. Use a compatible browser, or import a descriptor file or paste its text.');
    const formats=await BarcodeDetector.getSupportedFormats();
    if(!formats.includes('qr_code'))throw new Error('QR decoding is unavailable in this browser. Import the descriptor file instead.');
    return new BarcodeDetector({formats:['qr_code']});
  }
  async start(video,onResult,onStatus){
    this.stop();const generation=this.generation;
    try{
      if(!globalThis.isSecureContext||!navigator.mediaDevices?.getUserMedia)throw new Error('Camera access needs HTTPS or localhost and camera permission. Use descriptor-file import here.');
      const detector=await this.detector();if(generation!==this.generation)return;
      const stream=await navigator.mediaDevices.getUserMedia({audio:false,video:{facingMode:{ideal:'environment'},width:{ideal:1920},height:{ideal:1080}}});
      if(generation!==this.generation||!video.isConnected){stream.getTracks().forEach(track=>track.stop());return;}
      this.stream=stream;video.srcObject=stream;await video.play();
      if(generation!==this.generation)return;
      onStatus('Camera active. Hold the full descriptor QR inside the frame.',false,true);
      const tick=async()=>{
        if(generation!==this.generation||!video.isConnected)return;
        try{
          if(video.readyState>=2){
            const results=await detector.detect(video);
            if(generation!==this.generation)return;
            if(results.length>1){onStatus('Show one descriptor QR code at a time.',false,true);}
            else if(results[0]?.rawValue){const value=results[0].rawValue;this.stop();await onResult(value);return;}
          }
        }catch(_){if(generation!==this.generation)return;}
        this.timer=setTimeout(tick,200);
      };
      tick();
    }catch(err){
      if(generation!==this.generation)return;this.stop();
      const message=err.name==='NotAllowedError'?'Camera permission was denied or blocked by the preview. Open in a browser and allow camera access, or import a descriptor file.':err.name==='NotFoundError'?'No camera was found. Import a descriptor file instead.':err.name==='NotReadableError'?'The camera is busy or unavailable. Close other camera apps, or import a descriptor file.':err.message;
      onStatus(message,true,false);
    }
  }
  async readImage(file){
    if(file.size>12*1024*1024)throw new Error('Choose a QR image smaller than 12 MB.');
    if(!['image/png','image/jpeg','image/webp'].includes(file.type))throw new Error('Choose a PNG, JPEG, or WebP QR image.');
    const detector=await this.detector(),image=await createImageBitmap(file);
    try{
      if(image.width*image.height>24000000)throw new Error('Choose a smaller QR image (at most 24 megapixels).');
      const codes=await detector.detect(image);
      if(codes.length!==1||!codes[0].rawValue)throw new Error('One readable QR code is required. Try a sharper image or import the descriptor text file.');
      return codes[0].rawValue;
    }finally{image.close();}
  }
}
