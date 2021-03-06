using System;
using System.Collections.Generic;

namespace Steit.State.Event {
    public sealed class ListPopEventArgs<TItem, TList> : EventArgs where TList : IList<TItem>, IState {
        public UInt32 Tag { get; }
        public TItem Item { get; }
        public TList List { get; }

        public ListPopEventArgs(UInt32 tag, TItem item, TList list) {
            this.Tag = tag;
            this.Item = item;
            this.List = list;
        }
    }
}
