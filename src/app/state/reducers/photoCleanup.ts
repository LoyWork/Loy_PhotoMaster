import { Photo, PhotoId } from 'app/CommonTypes'
import { Action } from 'app/state/ActionType'
import { CHANGE_PHOTOS, DELETE_PHOTOS, EMPTY_TRASH, UPDATE_PHOTO_CLEANUP } from 'app/state/actionTypes'
import { AppState, PhotoCleanupSession } from 'app/state/StateTypes'

const createSession = (exact: boolean): PhotoCleanupSession => ({
    mode: exact ? 'exact' : 'deep', model: null, sensitivity: 'standard', result: null,
    selected: new Set(), page: 0, scrollTop: 0, expandedGroups: {}, hadGroups: false, notice: '',
})

function reconcile(session: PhotoCleanupSession, removed: Set<PhotoId>, updates: Map<PhotoId, Partial<Photo>>): PhotoCleanupSession {
    if (!session.result) return session
    let changed = false
    const groups = session.result.groups.map(group => ({ photos: group.photos.flatMap(item => {
        const id = item.photo.id
        if (removed.has(id)) { changed = true; return [] }
        const update = updates.get(id)
        if (!update) return [item]
        changed = true
        return [{ ...item, photo: { ...item.photo, ...update } }]
    }) })).filter(group => group.photos.length > 1)
    if (!changed) return session
    const selected = new Set<PhotoId>()
    for (const group of groups) {
        // A retained photo may have been removed on another page. Keep the remainder
        // until the user chooses again, rather than leave an entire group selected.
        if (group.photos.every(item => session.selected.has(item.photo.id))) continue
        for (const item of group.photos) if (session.selected.has(item.photo.id)) selected.add(item.photo.id)
    }
    return { ...session, result: { ...session.result, groups }, selected }
}

export function photoCleanup(state: AppState['photoCleanup'] = {
    similar: createSession(false), duplicates: createSession(true),
}, action: Action): AppState['photoCleanup'] {
    if (action.type === UPDATE_PHOTO_CLEANUP) {
        const { kind, update } = action.payload
        return { ...state, [kind]: { ...state[kind], ...update } }
    }
    const removed = new Set<PhotoId>()
    const updates = new Map<PhotoId, Partial<Photo>>()
    switch (action.type) {
        case CHANGE_PHOTOS:
            for (const photo of action.payload.photos) {
                if (action.payload.update.trashed) removed.add(photo.id)
                else updates.set(photo.id, action.payload.update)
            }
            break
        case DELETE_PHOTOS:
            action.payload.photoIds.forEach(id => removed.add(id)); break
        case EMPTY_TRASH:
            action.payload.trashedPhotoIds.forEach(id => removed.add(id)); break
        default: return state
    }
    const similar = reconcile(state.similar, removed, updates)
    const duplicates = reconcile(state.duplicates, removed, updates)
    return similar === state.similar && duplicates === state.duplicates ? state : { similar, duplicates }
}
