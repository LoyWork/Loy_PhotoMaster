import { Photo, PhotoId, SimilarScanResult } from 'app/CommonTypes'
import { changePhotosAction, deletePhotosAction, emptyTrashAction, openSettingsAction, updatePhotoCleanupAction } from 'app/state/actions'
import { photoCleanup } from 'app/state/reducers/photoCleanup'

const id = (value: number) => value as PhotoId
const result = { mode: 'deep', total: 20, skipped: 0, cancelled: false,
    groups: [[1, 2, 3], [4, 5]].map(ids => ({ photos: ids.map(value => ({ photo: { id: id(value), flag: false } })) })),
} as SimilarScanResult
const initial = () => photoCleanup(undefined, updatePhotoCleanupAction('similar', {
    result, selected: new Set([id(2), id(3), id(5)]), page: 1, scrollTop: 650, expandedGroups: { 1: 48 },
}))

test('page navigation preserves independent cleanup results, choices and browsing position', () => {
    const before = initial()
    const after = photoCleanup(before, openSettingsAction())
    expect(after).toBe(before)
    const separate = photoCleanup(after, updatePhotoCleanupAction('duplicates', {
        result: { ...result, mode: 'exact' }, selected: new Set([id(1)]), scrollTop: 100,
    }))
    expect(separate.similar).toBe(before.similar)
    expect(separate.similar.scrollTop).toBe(650)
    expect(separate.similar.expandedGroups).toEqual({ 1: 48 })
    expect([...separate.duplicates.selected]).toEqual([1])
})

test('trashing on either page removes photos from both caches and protects a removed keeper', () => {
    const before = photoCleanup(initial(), updatePhotoCleanupAction('duplicates', { result }))
    const after = photoCleanup(before, changePhotosAction([{ id: id(1) }] as Photo[], { trashed: true }))
    for (const kind of ['similar', 'duplicates'] as const) {
        expect(after[kind].result!.groups[0].photos.map(item => item.photo.id)).toEqual([2, 3])
    }
    expect([...after.similar.selected]).toEqual([5])
    expect(before.similar.result).toBe(result)
    expect([...before.similar.selected]).toEqual([2, 3, 5])
})

test('permanent deletion and empty trash remove singletons and orphaned selections', () => {
    const after = photoCleanup(initial(), deletePhotosAction([id(2), id(3)]))
    expect(after.similar.result!.groups.map(group => group.photos.map(item => item.photo.id))).toEqual([[4, 5]])
    expect([...after.similar.selected]).toEqual([5])
    const emptied = photoCleanup(after, emptyTrashAction([id(5)]))
    expect(emptied.similar.result!.groups).toEqual([])
    expect(emptied.similar.selected.size).toBe(0)
})

test('metadata edits update cached photos while unrelated changes preserve the cache', () => {
    const before = initial()
    expect(photoCleanup(before, deletePhotosAction([id(99)]))).toBe(before)
    const after = photoCleanup(before, changePhotosAction([{ id: id(1) }] as Photo[], { flag: true }))
    expect(after.similar.result!.groups[0].photos[0].photo.flag).toBe(true)
    expect(after.similar.scrollTop).toBe(650)
    expect([...after.similar.selected]).toEqual([2, 3, 5])
    expect(result.groups[0].photos[0].photo.flag).toBe(false)
})

test('explicit whole-group selection survives metadata edits and partial removal', () => {
    const before = photoCleanup(initial(), updatePhotoCleanupAction('similar', {
        selected: new Set([id(1), id(2), id(3), id(5)]),
    }))
    const edited = photoCleanup(before, changePhotosAction([{ id: id(1) }] as Photo[], { flag: true }))
    expect([...edited.similar.selected]).toEqual([1, 2, 3, 5])
    const partial = photoCleanup(edited, deletePhotosAction([id(1)]))
    expect([...partial.similar.selected]).toEqual([2, 3, 5])
    const trashed = photoCleanup(partial, changePhotosAction([{ id: id(2) }, { id: id(3) }] as Photo[], { trashed: true }))
    expect(trashed.similar.result!.groups.map(group => group.photos.map(item => item.photo.id))).toEqual([[4, 5]])
    expect([...trashed.similar.selected]).toEqual([5])
})
