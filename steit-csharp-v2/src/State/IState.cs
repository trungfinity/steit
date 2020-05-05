using System;

using Steit.Codec;

namespace Steit.State {
    public interface IState {
        Path Path { get; }

        WireType? GetWireType(UInt32 tag);
        IState? GetNested(UInt32 tag);

        void ReplaceAt(UInt32 tag, WireType wireType, IReader reader, bool shouldNotify);

        bool IsList();
        void ReplayListPush(IReader reader);
        void ReplayListPop();

        bool IsMap();
        void ReplayMapInsert(IReader reader);
        void ReplayMapRemove(IReader reader);
    }
}