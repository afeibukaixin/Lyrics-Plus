#import <CoreAudio/AudioHardware.h>
#import <CoreAudio/AudioHardwareTapping.h>
#import <CoreAudio/CATapDescription.h>
#import <Foundation/Foundation.h>

#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef void (*LyricsPlusSpectrumCallback)(const float *samples,
                                           uint32_t sample_count,
                                           double sample_rate,
                                           void *context);

enum {
    LYRICS_PLUS_SPECTRUM_UNSUPPORTED = -10000,
    LYRICS_PLUS_SPECTRUM_INVALID_ARGUMENT = -10001,
    LYRICS_PLUS_SPECTRUM_FORMAT_UNSUPPORTED = -10002,
};

typedef struct LyricsPlusAudioTap {
    AudioObjectID tap_id;
    AudioObjectID aggregate_device_id;
    AudioDeviceIOProcID io_proc_id;
    dispatch_queue_t io_queue;
    LyricsPlusSpectrumCallback callback;
    void *context;
    double sample_rate;
    uint32_t scratch_capacity;
    float *scratch;
    AudioObjectID *process_ids;
    uint32_t process_count;
} LyricsPlusAudioTap;

static NSString *lyrics_plus_audio_key(const char *key) {
    return [NSString stringWithUTF8String:key];
}

static int lyrics_plus_compare_audio_object_ids(const void *left, const void *right) {
    AudioObjectID left_id = *(const AudioObjectID *)left;
    AudioObjectID right_id = *(const AudioObjectID *)right;
    return left_id < right_id ? -1 : (left_id > right_id ? 1 : 0);
}

static BOOL lyrics_plus_process_ids_for_bundle(const char *bundle_id,
                                               AudioObjectID **out_ids,
                                               uint32_t *out_count) {
    if (!bundle_id || !out_ids || !out_count) return NO;

    AudioObjectPropertyAddress list_address = {
        kAudioHardwarePropertyProcessObjectList,
        kAudioObjectPropertyScopeGlobal,
        kAudioObjectPropertyElementMain,
    };
    UInt32 list_size = 0;
    if (AudioObjectGetPropertyDataSize(kAudioObjectSystemObject,
                                       &list_address,
                                       0,
                                       NULL,
                                       &list_size) != noErr ||
        list_size < sizeof(AudioObjectID)) {
        return NO;
    }

    uint32_t capacity = list_size / sizeof(AudioObjectID);
    AudioObjectID *all_ids = calloc(capacity, sizeof(AudioObjectID));
    if (!all_ids) return NO;

    if (AudioObjectGetPropertyData(kAudioObjectSystemObject,
                                   &list_address,
                                   0,
                                   NULL,
                                   &list_size,
                                   all_ids) != noErr) {
        free(all_ids);
        return NO;
    }

    NSString *expected_bundle_id = [NSString stringWithUTF8String:bundle_id];
    AudioObjectID *matching_ids = calloc(capacity, sizeof(AudioObjectID));
    if (!matching_ids) {
        free(all_ids);
        return NO;
    }

    uint32_t matching_count = 0;
    uint32_t all_count = list_size / sizeof(AudioObjectID);
    for (uint32_t index = 0; index < all_count; index++) {
        AudioObjectPropertyAddress bundle_address = {
            kAudioProcessPropertyBundleID,
            kAudioObjectPropertyScopeGlobal,
            kAudioObjectPropertyElementMain,
        };
        CFStringRef process_bundle_id = NULL;
        UInt32 bundle_size = sizeof(process_bundle_id);
        OSStatus status = AudioObjectGetPropertyData(all_ids[index],
                                                     &bundle_address,
                                                     0,
                                                     NULL,
                                                     &bundle_size,
                                                     &process_bundle_id);
        if (status == noErr && process_bundle_id &&
            CFStringCompare(process_bundle_id,
                            (__bridge CFStringRef)expected_bundle_id,
                            0) == kCFCompareEqualTo) {
            matching_ids[matching_count++] = all_ids[index];
        }
        if (process_bundle_id) CFRelease(process_bundle_id);
    }

    free(all_ids);
    if (matching_count == 0) {
        free(matching_ids);
        return NO;
    }

    qsort(matching_ids,
          matching_count,
          sizeof(AudioObjectID),
          lyrics_plus_compare_audio_object_ids);
    *out_ids = matching_ids;
    *out_count = matching_count;
    return YES;
}

static BOOL lyrics_plus_device_is_alive(AudioObjectID device_id) {
    AudioObjectPropertyAddress address = {
        kAudioDevicePropertyDeviceIsAlive,
        kAudioObjectPropertyScopeGlobal,
        kAudioObjectPropertyElementMain,
    };
    UInt32 size = sizeof(UInt32);
    UInt32 alive = 0;
    if (AudioObjectGetPropertyData(device_id, &address, 0, NULL, &size, &alive) != noErr) {
        return YES;
    }
    return alive != 0;
}

static BOOL lyrics_plus_wait_for_device(AudioObjectID device_id) {
    for (int attempt = 0; attempt < 50; attempt++) {
        if (lyrics_plus_device_is_alive(device_id)) return YES;
        usleep(20 * 1000);
    }
    return NO;
}

static BOOL lyrics_plus_audio_format(AudioObjectID object_id,
                                     AudioObjectPropertySelector selector,
                                     AudioObjectPropertyScope scope,
                                     double *sample_rate,
                                     uint32_t *channels) {
    AudioObjectPropertyAddress address = {
        selector,
        scope,
        kAudioObjectPropertyElementMain,
    };
    AudioStreamBasicDescription format = {0};
    UInt32 size = sizeof(format);
    if (AudioObjectGetPropertyData(object_id, &address, 0, NULL, &size, &format) != noErr) {
        return NO;
    }
    if (format.mSampleRate <= 0 || format.mChannelsPerFrame == 0) return NO;
    if ((format.mFormatFlags & kAudioFormatFlagIsFloat) == 0 ||
        format.mBitsPerChannel != 32) {
        return NO;
    }
    *sample_rate = format.mSampleRate;
    *channels = format.mChannelsPerFrame;
    return YES;
}

static NSString *lyrics_plus_tap_uid(AudioObjectID tap_id) {
    AudioObjectPropertyAddress address = {
        kAudioTapPropertyUID,
        kAudioObjectPropertyScopeGlobal,
        kAudioObjectPropertyElementMain,
    };
    CFStringRef uid = NULL;
    UInt32 size = sizeof(uid);
    if (AudioObjectGetPropertyData(tap_id, &address, 0, NULL, &size, &uid) != noErr || !uid) {
        return nil;
    }
    return CFBridgingRelease(uid);
}

static void lyrics_plus_emit_audio(LyricsPlusAudioTap *tap,
                                   const AudioBufferList *input_data) {
    if (!tap || !input_data || input_data->mNumberBuffers == 0 || !tap->callback) return;

    const AudioBuffer *first = &input_data->mBuffers[0];
    if (!first->mData || first->mDataByteSize < sizeof(float)) return;

    if (input_data->mNumberBuffers == 1 && first->mNumberChannels == 1) {
        uint32_t sample_count = first->mDataByteSize / sizeof(float);
        tap->callback((const float *)first->mData, sample_count, tap->sample_rate, tap->context);
        return;
    }

    uint32_t buffer_count = input_data->mNumberBuffers;
    uint32_t frame_count = first->mDataByteSize / sizeof(float);
    if (first->mNumberChannels > 1) {
        frame_count /= first->mNumberChannels;
    }
    if (frame_count == 0 || frame_count > tap->scratch_capacity) return;

    memset(tap->scratch, 0, frame_count * sizeof(float));
    uint32_t contributing_channels = 0;
    for (uint32_t buffer_index = 0; buffer_index < buffer_count; buffer_index++) {
        const AudioBuffer *buffer = &input_data->mBuffers[buffer_index];
        if (!buffer->mData || buffer->mNumberChannels == 0) continue;
        uint32_t channels = buffer->mNumberChannels;
        uint32_t available_frames = buffer->mDataByteSize / sizeof(float) / channels;
        if (available_frames < frame_count) frame_count = available_frames;
        const float *samples = (const float *)buffer->mData;
        for (uint32_t frame = 0; frame < frame_count; frame++) {
            float sum = 0.0f;
            for (uint32_t channel = 0; channel < channels; channel++) {
                sum += samples[frame * channels + channel];
            }
            tap->scratch[frame] += sum / (float)channels;
        }
        contributing_channels++;
    }
    if (contributing_channels == 0 || frame_count == 0) return;
    if (contributing_channels > 1) {
        float divisor = (float)contributing_channels;
        for (uint32_t frame = 0; frame < frame_count; frame++) {
            tap->scratch[frame] /= divisor;
        }
    }
    tap->callback(tap->scratch, frame_count, tap->sample_rate, tap->context);
}

static void lyrics_plus_destroy_tap(LyricsPlusAudioTap *tap) API_AVAILABLE(macos(14.2));

static void lyrics_plus_destroy_tap(LyricsPlusAudioTap *tap) {
    if (!tap) return;
    if (tap->aggregate_device_id != kAudioObjectUnknown && tap->io_proc_id) {
        AudioDeviceStop(tap->aggregate_device_id, tap->io_proc_id);
        AudioDeviceDestroyIOProcID(tap->aggregate_device_id, tap->io_proc_id);
    }
    if (tap->aggregate_device_id != kAudioObjectUnknown) {
        AudioHardwareDestroyAggregateDevice(tap->aggregate_device_id);
    }
    if (tap->tap_id != kAudioObjectUnknown) {
        AudioHardwareDestroyProcessTap(tap->tap_id);
    }
    tap->io_queue = nil;
    free(tap->scratch);
    free(tap->process_ids);
    free(tap);
}

static int lyrics_plus_start_tap(const char *bundle_id,
                                 LyricsPlusSpectrumCallback callback,
                                 void *context,
                                 LyricsPlusAudioTap **out_tap) API_AVAILABLE(macos(14.2));

static int lyrics_plus_start_tap(const char *bundle_id,
                                 LyricsPlusSpectrumCallback callback,
                                 void *context,
                                 LyricsPlusAudioTap **out_tap) {
    if (!bundle_id || !callback || !out_tap) return LYRICS_PLUS_SPECTRUM_INVALID_ARGUMENT;

    AudioObjectID *process_ids = NULL;
    uint32_t process_count = 0;
    if (!lyrics_plus_process_ids_for_bundle(bundle_id, &process_ids, &process_count)) {
        return kAudioHardwareBadDeviceError;
    }

    NSMutableArray<NSNumber *> *process_objects = [NSMutableArray arrayWithCapacity:process_count];
    for (uint32_t index = 0; index < process_count; index++) {
        [process_objects addObject:@(process_ids[index])];
    }

    CATapDescription *description = [[CATapDescription alloc] initMonoMixdownOfProcesses:process_objects];
    description.name = @"Lyrics Plus Spectrum";
    description.privateTap = YES;
    description.muteBehavior = CATapUnmuted;
    AudioObjectID tap_id = kAudioObjectUnknown;
    OSStatus status = AudioHardwareCreateProcessTap(description, &tap_id);
    if (status != noErr) {
        free(process_ids);
        return status;
    }

    NSString *tap_uid = lyrics_plus_tap_uid(tap_id);
    if (!tap_uid) {
        AudioHardwareDestroyProcessTap(tap_id);
        free(process_ids);
        return kAudioHardwareBadObjectError;
    }
    NSString *aggregate_uid = [NSUUID UUID].UUIDString;
    NSDictionary *tap_entry = @{
        lyrics_plus_audio_key(kAudioSubTapUIDKey): tap_uid,
        lyrics_plus_audio_key(kAudioSubTapDriftCompensationKey): @YES,
    };
    NSDictionary *aggregate_description = @{
        lyrics_plus_audio_key(kAudioAggregateDeviceNameKey): @"Lyrics Plus Spectrum",
        lyrics_plus_audio_key(kAudioAggregateDeviceUIDKey): aggregate_uid,
        lyrics_plus_audio_key(kAudioAggregateDeviceIsPrivateKey): @YES,
        lyrics_plus_audio_key(kAudioAggregateDeviceTapListKey): @[tap_entry],
    };

    AudioObjectID aggregate_device_id = kAudioObjectUnknown;
    status = AudioHardwareCreateAggregateDevice((__bridge CFDictionaryRef)aggregate_description,
                                                 &aggregate_device_id);
    if (status != noErr) {
        AudioHardwareDestroyProcessTap(tap_id);
        free(process_ids);
        return status;
    }
    if (!lyrics_plus_wait_for_device(aggregate_device_id)) {
        AudioHardwareDestroyAggregateDevice(aggregate_device_id);
        AudioHardwareDestroyProcessTap(tap_id);
        free(process_ids);
        return kAudioHardwareNotRunningError;
    }

    double sample_rate = 0;
    uint32_t channels = 0;
    if (!lyrics_plus_audio_format(tap_id,
                                  kAudioTapPropertyFormat,
                                  kAudioObjectPropertyScopeGlobal,
                                  &sample_rate,
                                  &channels)) {
        AudioHardwareDestroyAggregateDevice(aggregate_device_id);
        AudioHardwareDestroyProcessTap(tap_id);
        free(process_ids);
        return LYRICS_PLUS_SPECTRUM_FORMAT_UNSUPPORTED;
    }

    LyricsPlusAudioTap *tap = calloc(1, sizeof(LyricsPlusAudioTap));
    if (!tap) {
        AudioHardwareDestroyAggregateDevice(aggregate_device_id);
        AudioHardwareDestroyProcessTap(tap_id);
        free(process_ids);
        return kAudioHardwareUnspecifiedError;
    }
    tap->tap_id = tap_id;
    tap->aggregate_device_id = aggregate_device_id;
    tap->callback = callback;
    tap->context = context;
    tap->sample_rate = sample_rate;
    tap->scratch_capacity = 8192;
    tap->scratch = calloc(tap->scratch_capacity, sizeof(float));
    tap->process_ids = process_ids;
    tap->process_count = process_count;
    tap->io_queue = dispatch_queue_create("com.xiaoafei.lyrics-plus.spectrum-io", DISPATCH_QUEUE_SERIAL);
    if (!tap->scratch || !tap->io_queue) {
        lyrics_plus_destroy_tap(tap);
        return kAudioHardwareUnspecifiedError;
    }

    status = AudioDeviceCreateIOProcIDWithBlock(&tap->io_proc_id,
                                                aggregate_device_id,
                                                tap->io_queue,
                                                ^(const AudioTimeStamp *in_now,
                                                  const AudioBufferList *input_data,
                                                  const AudioTimeStamp *in_input_time,
                                                  AudioBufferList *output_data,
                                                  const AudioTimeStamp *in_output_time) {
        (void)in_now;
        (void)in_input_time;
        (void)output_data;
        (void)in_output_time;
        lyrics_plus_emit_audio(tap, input_data);
    });
    if (status != noErr) {
        lyrics_plus_destroy_tap(tap);
        return status;
    }

    status = AudioDeviceStart(aggregate_device_id, tap->io_proc_id);
    if (status != noErr) {
        lyrics_plus_destroy_tap(tap);
        return status;
    }

    *out_tap = tap;
    (void)channels;
    return noErr;
}

int lyrics_plus_audio_tap_start(const char *bundle_id,
                                LyricsPlusSpectrumCallback callback,
                                void *context,
                                void **out_tap) {
    @autoreleasepool {
        if (@available(macOS 14.2, *)) {
            // Process Tap API is available on this runtime.
        } else {
            return LYRICS_PLUS_SPECTRUM_UNSUPPORTED;
        }
        if (!out_tap) return LYRICS_PLUS_SPECTRUM_INVALID_ARGUMENT;
        LyricsPlusAudioTap *tap = NULL;
        int status = LYRICS_PLUS_SPECTRUM_UNSUPPORTED;
        if (@available(macOS 14.2, *)) {
            status = lyrics_plus_start_tap(bundle_id, callback, context, &tap);
        }
        if (status == noErr) *out_tap = tap;
        return status;
    }
}

void lyrics_plus_audio_tap_stop(void *opaque_tap) {
    if (!opaque_tap) return;
    if (@available(macOS 14.2, *)) {
        lyrics_plus_destroy_tap((LyricsPlusAudioTap *)opaque_tap);
    }
}

int lyrics_plus_audio_tap_matches_bundle(void *opaque_tap, const char *bundle_id) {
    @autoreleasepool {
        if (@available(macOS 14.2, *)) {
            // Process list inspection is safe on this runtime.
        } else {
            return 0;
        }
        LyricsPlusAudioTap *tap = (LyricsPlusAudioTap *)opaque_tap;
        if (!tap || !bundle_id) return 0;
        AudioObjectID *current_ids = NULL;
        uint32_t current_count = 0;
        if (!lyrics_plus_process_ids_for_bundle(bundle_id, &current_ids, &current_count)) return 0;
        BOOL same = current_count == tap->process_count;
        if (same) {
            for (uint32_t index = 0; index < current_count; index++) {
                if (current_ids[index] != tap->process_ids[index]) {
                    same = NO;
                    break;
                }
            }
        }
        free(current_ids);
        return same ? 1 : 0;
    }
}
