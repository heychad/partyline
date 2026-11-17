#import <Foundation/Foundation.h>
#import <CoreAudio/CoreAudio.h>
// Process activity only. No audio samples, windows, browser history, or accessibility data.
char *partyline_microphone_processes(void) {
 @autoreleasepool {
  if (@available(macOS 14.2,*)) {
   AudioObjectPropertyAddress a={kAudioHardwarePropertyProcessObjectList,kAudioObjectPropertyScopeGlobal,kAudioObjectPropertyElementMain};UInt32 size=0;
   if(AudioObjectGetPropertyDataSize(kAudioObjectSystemObject,&a,0,NULL,&size)!=noErr) return strdup("{\"error\":\"Microphone activity is unavailable on this Mac.\"}");
   NSMutableData *data=[NSMutableData dataWithLength:size];
   if(AudioObjectGetPropertyData(kAudioObjectSystemObject,&a,0,NULL,&size,data.mutableBytes)!=noErr)return strdup("{\"error\":\"Could not inspect microphone activity.\"}");
   NSMutableSet *bundles=[NSMutableSet set];AudioObjectID *ids=data.mutableBytes;
   for(UInt32 i=0;i<size/sizeof(AudioObjectID);i++){
    UInt32 running=0,n=sizeof(running);AudioObjectPropertyAddress input={kAudioProcessPropertyIsRunningInput,kAudioObjectPropertyScopeGlobal,kAudioObjectPropertyElementMain};
    if(AudioObjectGetPropertyData(ids[i],&input,0,NULL,&n,&running)!=noErr||!running)continue;
    CFStringRef bundle=NULL;n=sizeof(bundle);AudioObjectPropertyAddress bid={kAudioProcessPropertyBundleID,kAudioObjectPropertyScopeGlobal,kAudioObjectPropertyElementMain};
    if(AudioObjectGetPropertyData(ids[i],&bid,0,NULL,&n,&bundle)==noErr&&bundle){[bundles addObject:(__bridge NSString*)bundle];CFRelease(bundle);}
   }
   NSData *json=[NSJSONSerialization dataWithJSONObject:@{@"bundles":bundles.allObjects} options:0 error:NULL];
   return strdup([[NSString alloc]initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
  }
  return strdup("{\"error\":\"Meeting detection requires macOS 14.2 or later.\"}");
 }
}
void partyline_free_detection(char *value){free(value);}
