import { PhotoId, SimilarPhotoGroup } from 'app/CommonTypes'
import { keepOnlySimilarPhoto, selectEntireSimilarGroup, selectSimilarPhoto } from 'app/util/SimilarPhotoSelection'

const id = (value: number) => value as PhotoId
const group = { photos: [1, 2, 3].map(value => ({ photo: { id: id(value) } })) } as SimilarPhotoGroup

test('selection preserves the last kept photo and keeps choices in other groups', () => {
    const original = new Set([id(9)])
    const first = selectSimilarPhoto(original, group, id(1))
    const second = selectSimilarPhoto(first, group, id(2))
    expect([...selectSimilarPhoto(second, group, id(3))]).toEqual([9, 1, 2])
    expect([...selectSimilarPhoto(second, group, id(1))]).toEqual([9, 2])
    expect([...original]).toEqual([9])
})

test('keeping a different photo swaps the retained copy without selecting unrelated photos', () => {
    const original = new Set([id(9), id(1)])
    expect([...keepOnlySimilarPhoto(original, group, id(1))].sort()).toEqual([2, 3, 9])
    expect([...keepOnlySimilarPhoto(original, group, id(2))].sort()).toEqual([1, 3, 9])
    expect([...keepOnlySimilarPhoto(original, group, id(7))]).toEqual([...original])
})

test('explicit group selection includes every photo and can be changed back to keeping a photo', () => {
    const original = new Set([id(9), id(2)])
    const selected = selectEntireSimilarGroup(original, group)
    expect([...selected].sort()).toEqual([1, 2, 3, 9])
    expect([...original]).toEqual([9, 2])
    expect([...selectEntireSimilarGroup(selected, group)].sort()).toEqual([1, 2, 3, 9])
    expect([...selectSimilarPhoto(selected, group, id(1))].sort()).toEqual([2, 3, 9])
    expect([...keepOnlySimilarPhoto(selected, group, id(2))].sort()).toEqual([1, 3, 9])
})
