ObjC.import("Foundation")

function isNil(value) {
  if (value === null || value === undefined) return true
  try { return Boolean(value.isNil()) } catch (_) { return false }
}

function unwrap(value) {
  if (isNil(value)) return null
  try { return ObjC.unwrap(value) } catch (_) { return null }
}

function valueFor(info, key) {
  return isNil(info) ? null : unwrap(info.objectForKey(key))
}

function dateSeconds(value) {
  if (isNil(value)) return null
  const seconds = Number(value.timeIntervalSince1970)
  return Number.isFinite(seconds) && seconds >= 0 ? seconds : null
}

function nativeBlock(callback) {
  // JXA cannot infer object-valued blocks declared only by a private framework.
  const holder = $.NSPredicate.predicateWithBlock(callback)
  return { holder, block: holder.valueForKey("block") }
}

function requestFor(bundleId) {
  const origin = $.NSClassFromString("MROrigin")
  const player = $.NSClassFromString("MRPlayer")
  const pathClass = $.NSClassFromString("MRPlayerPath")
  const requestClass = $.NSClassFromString("MRNowPlayingRequest")
  if (isNil(origin) || isNil(player) || isNil(pathClass) || isNil(requestClass)) {
    throw new Error("MediaRemote player-path API is unavailable")
  }
  const path = pathClass.alloc.initWithOriginBundleIdentifierPlayer(
    origin.localOrigin, bundleId, player.defaultPlayer
  )
  if (isNil(path)) throw new Error("Cannot create MediaRemote player path")
  const request = requestClass.alloc.initWithPlayerPath(path)
  if (isNil(request)) throw new Error("Cannot create MediaRemote request")
  return { path, request }
}

function startQuery(bundleId, includeArtwork, queue, keepAlive) {
  const { path, request } = requestFor(bundleId)
  const state = {
    bundleIdentifier: bundleId,
    info: null,
    infoDone: false,
    infoError: null,
    playing: null,
    playingDone: false,
    playingError: null,
    activityDate: null,
    dateDone: false,
    artwork: null,
    artworkDone: !includeArtwork,
  }
  keepAlive.push(path, request)

  const infoSelector = $.NSSelectorFromString("requestNowPlayingInfoWithCompletion:")
  const playingSelector = $.NSSelectorFromString("requestIsPlayingOnQueue:completion:")
  if (!request.respondsToSelector(infoSelector) || !request.respondsToSelector(playingSelector)) {
    throw new Error("MediaRemote per-player query API is unavailable")
  }

  const infoCallback = ObjC.block(["void", ["id", "id"]], function(info, error) {
    state.infoDone = true
    state.info = isNil(info) ? null : info
    state.infoError = isNil(error) ? null : Number(error.code)
  })
  const infoBridge = nativeBlock(infoCallback)
  keepAlive.push(infoCallback, infoBridge.holder, infoBridge.block)
  $.objc_msgSend(request, infoSelector, infoBridge.block, null)

  const playingCallback = ObjC.block(["void", ["bool", "id"]], function(playing, error) {
    state.playingDone = true
    state.playingError = isNil(error) ? null : Number(error.code)
    state.playing = isNil(error) ? Boolean(playing) : null
  })
  keepAlive.push(playingCallback)
  $.objc_msgSend(request, playingSelector, queue, playingCallback)

  const dateSelector = $.NSSelectorFromString("requestLastPlayingDateWithCompletion:")
  if (request.respondsToSelector(dateSelector)) {
    const dateCallback = ObjC.block(["void", ["id", "id"]], function(date, _) {
      state.dateDone = true
      state.activityDate = dateSeconds(date)
    })
    const dateBridge = nativeBlock(dateCallback)
    keepAlive.push(dateCallback, dateBridge.holder, dateBridge.block)
    $.objc_msgSend(request, dateSelector, dateBridge.block, null)
  } else {
    state.dateDone = true
  }

  if (includeArtwork) {
    const artworkSelector = $.NSSelectorFromString("requestNowPlayingItemArtworkWithCompletion:")
    if (request.respondsToSelector(artworkSelector)) {
      const artworkCallback = ObjC.block(["void", ["id", "id"]], function(artwork, _) {
        state.artworkDone = true
        if (isNil(artwork)) return
        try {
          const selector = $.NSSelectorFromString("imageData")
          const data = artwork.respondsToSelector(selector) ? artwork.imageData : artwork
          if (!isNil(data)) state.artwork = unwrap(data.base64EncodedStringWithOptions(0))
        } catch (_) {}
      })
      const artworkBridge = nativeBlock(artworkCallback)
      keepAlive.push(artworkCallback, artworkBridge.holder, artworkBridge.block)
      $.objc_msgSend(request, artworkSelector, artworkBridge.block, null)
    } else {
      state.artworkDone = true
    }
  }
  return state
}

function query(bundleIds, includeArtwork) {
  ObjC.bindFunction("dispatch_queue_create", ["id", ["string", "void*"]])
  ObjC.bindFunction("objc_msgSend", ["void", ["id", "selector", "id", "id"]])
  const queue = $.dispatch_queue_create("lyrics-plus.targeted-media", null)
  const keepAlive = [queue]
  const states = bundleIds.map(bundleId => {
    try { return startQuery(bundleId, includeArtwork, queue, keepAlive) }
    catch (_) { return { bundleIdentifier: bundleId, failed: true } }
  })
  const deadline = Date.now() + (includeArtwork ? 900 : 1250)
  while (Date.now() < deadline && states.some(state =>
    !state.failed && (!state.infoDone ||
      (state.infoError === null && !state.playingDone) ||
      (state.infoError === null && !state.dateDone) ||
      (includeArtwork && state.infoError === null && !state.artworkDone))
  )) {
    $.NSRunLoop.currentRunLoop.runUntilDate($.NSDate.dateWithTimeIntervalSinceNow(0.025))
  }
  return states.map(state => {
    if (state.failed || !state.infoDone) {
      return { bundleIdentifier: state.bundleIdentifier, status: "error" }
    }
    if (state.infoError === 1) {
      return { bundleIdentifier: state.bundleIdentifier, status: "missing" }
    }
    if (state.infoError !== null || !state.playingDone ||
        state.playingError !== null || state.playing === null) {
      return { bundleIdentifier: state.bundleIdentifier, status: "error" }
    }
    if (isNil(state.info)) {
      return { bundleIdentifier: state.bundleIdentifier, status: "missing" }
    }
    const title = valueFor(state.info, "kMRMediaRemoteNowPlayingInfoTitle")
    if (typeof title !== "string" || !title.trim()) {
      return { bundleIdentifier: state.bundleIdentifier, status: "missing" }
    }
    return {
      bundleIdentifier: state.bundleIdentifier,
      status: "ok",
      title,
      artist: valueFor(state.info, "kMRMediaRemoteNowPlayingInfoArtist"),
      album: valueFor(state.info, "kMRMediaRemoteNowPlayingInfoAlbum"),
      duration: valueFor(state.info, "kMRMediaRemoteNowPlayingInfoDuration"),
      elapsedTime: valueFor(state.info, "kMRMediaRemoteNowPlayingInfoElapsedTime"),
      timestamp: dateSeconds(state.info.objectForKey("kMRMediaRemoteNowPlayingInfoTimestamp")),
      playing: state.playing,
      activityDate: state.activityDate || dateSeconds(
        state.info.objectForKey("kMRMediaRemoteNowPlayingInfoTimestamp")
      ),
      artworkData: state.artwork,
    }
  })
}

function run(args) {
  const framework = $.NSBundle.bundleWithPath("/System/Library/PrivateFrameworks/MediaRemote.framework")
  if (isNil(framework) || !framework.load) throw new Error("MediaRemote is unavailable")
  if (args[0] === "query") return JSON.stringify(query(args.slice(1), false))
  if (args[0] === "artwork") return JSON.stringify(query(args.slice(1, 2), true))
  throw new Error("Invalid targeted-media command")
}
