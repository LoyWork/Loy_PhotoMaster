import { PhotoId, PhotoSectionId } from 'app/CommonTypes'
import { emptyTrashAction, fetchSectionsAction } from 'app/state/actions'
import { data } from 'app/state/reducers/data'

test('partial trash cleanup preserves failed photos and previous render state', () => {
    const sectionId = '2026-11-12' as PhotoSectionId
    const section = { id: sectionId, count: 2, photoIds: [1, 2] as PhotoId[], photoData: {
        1: { id: 1 }, 2: { id: 2 },
    } } as any
    const before = data(undefined, fetchSectionsAction.success({ sections: [section] }))
    const after = data(before, emptyTrashAction([1 as PhotoId]))
    expect(after.sections.photoCount).toBe(1)
    expect(after.sections.ids).toEqual([sectionId])
    expect((after.sections.byId[sectionId] as any).photoIds).toEqual([2])
    expect((before.sections.byId[sectionId] as any).photoIds).toEqual([1, 2])
    expect((before.sections.byId[sectionId] as any).photoData[1]).toBeDefined()
    const done = data(after, emptyTrashAction([2 as PhotoId]))
    expect(done.sections.photoCount).toBe(0)
    expect(done.sections.ids).toEqual([])
})
