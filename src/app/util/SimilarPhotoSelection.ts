import { PhotoId, SimilarPhotoGroup } from 'app/CommonTypes'

/** Individual selection keeps one photo; selecting the entire group is an explicit action. */
export function selectSimilarPhoto(selected: ReadonlySet<PhotoId>, group: SimilarPhotoGroup, id: PhotoId): Set<PhotoId> {
    const next = new Set(selected)
    if (!group.photos.some(item => item.photo.id === id)) return next
    if (next.has(id)) next.delete(id)
    else if (group.photos.some(item => item.photo.id !== id && !next.has(item.photo.id))) next.add(id)
    return next
}

export function keepOnlySimilarPhoto(selected: ReadonlySet<PhotoId>, group: SimilarPhotoGroup, id: PhotoId): Set<PhotoId> {
    const next = new Set(selected)
    if (!group.photos.some(item => item.photo.id === id)) return next
    for (const item of group.photos) {
        if (item.photo.id === id) next.delete(id)
        else next.add(item.photo.id)
    }
    return next
}

export function selectEntireSimilarGroup(selected: ReadonlySet<PhotoId>, group: SimilarPhotoGroup): Set<PhotoId> {
    return new Set([...selected, ...group.photos.map(item => item.photo.id)])
}
