using System;
using System.Collections.Generic;

using Steit.Codec;
using Steit.Collections;

namespace Steit.State {
    public static class StateReplayer {
        public static void Replay<T>(ref T root, IReader reader) where T : IState {
            while (!reader.EndOfStream()) {
                var entry = LogEntry.Deserialize(reader.GetNested());
                Replay(ref root, entry);
            }
        }

        public static void Replay<T>(ref T root, LogEntry entry) where T : IState {
            var path = new List<UInt32>(GetPath(entry));
            var tag = 0U;

            if (entry.Tag == LogEntry.UpdateTag) {
                if (path.Count > 0) {
                    tag = path[path.Count - 1];
                    path.RemoveAt(path.Count - 1);
                } else {
                    // var reader = new ByteReader(entry.UpdateVariant!.Value);
                    var reader = new ByteReader(entry.UpdateVariant.Value);
                    root = StateFactory.Deserialize<T>(reader, root.Path);
                    return;
                }
            }

            var container = root.GetNested(path);

            if (container == null) {
                return;
            }

            switch (entry.Tag) {
                case LogEntry.UpdateTag: {
                        var wireType = container.GetWireType(tag);
                        if (wireType == null) { return; }
                        // var reader = new ByteReader(entry.UpdateVariant!.Value);
                        var reader = new ByteReader(entry.UpdateVariant.Value);
                        container.ReplaceAt(tag, wireType.Value, reader, shouldNotify: true);
                        break;
                    }

                case LogEntry.ListPushTag: {
                        // var reader = new ByteReader(entry.ListPushVariant!.Item);
                        var reader = new ByteReader(entry.ListPushVariant.Item);
                        container.ReplayListPush(reader);
                        break;
                    }

                case LogEntry.ListPopTag: {
                        container.ReplayListPop();
                        break;
                    }

                case LogEntry.MapRemoveTag: {
                        // var key = entry.MapRemoveVariant!.Key;
                        var key = entry.MapRemoveVariant.Key;
                        container.ReplayMapRemove(key);
                        break;
                    }

                default: break;
            }
        }

        private static Vector<UInt32> GetPath(LogEntry entry) {
            switch (entry.Tag) {
                // case LogEntry.UpdateTag: return entry.UpdateVariant!.FlattenPath;
                case LogEntry.UpdateTag: return entry.UpdateVariant.FlattenPath;
                // case LogEntry.ListPushTag: return entry.ListPushVariant!.FlattenPath;
                case LogEntry.ListPushTag: return entry.ListPushVariant.FlattenPath;
                // case LogEntry.ListPopTag: return entry.ListPopVariant!.FlattenPath;
                case LogEntry.ListPopTag: return entry.ListPopVariant.FlattenPath;
                // case LogEntry.MapRemoveTag: return entry.MapRemoveVariant!.FlattenPath;
                case LogEntry.MapRemoveTag: return entry.MapRemoveVariant.FlattenPath;
                default: throw new InvalidOperationException(String.Format("Unknown log entry tag {0}", entry.Tag));
            }
        }
    }
}
