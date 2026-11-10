import { SetStateAction, useCallback } from 'react'
import { useSelector } from 'react-redux'
import { updatePhotoCleanupAction } from 'app/state/actions'
import { AppState, PhotoCleanupKind, PhotoCleanupSession } from 'app/state/StateTypes'
import store from 'app/state/store'

export default function usePhotoCleanupSession(kind: PhotoCleanupKind) {
    const session = useSelector((state: AppState) => state.photoCleanup[kind])
    const set = useCallback(<K extends keyof PhotoCleanupSession>(key: K, value: SetStateAction<PhotoCleanupSession[K]>) => {
        const current = store.getState().photoCleanup[kind][key]
        const next = typeof value === 'function'
            ? (value as (previous: PhotoCleanupSession[K]) => PhotoCleanupSession[K])(current) : value
        store.dispatch(updatePhotoCleanupAction(kind, { [key]: next }))
    }, [kind])
    return { session, set }
}
