using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;

namespace ConvertZZ
{
    public class FastReplace
    {
        SortedDictionary<string, string> Dic_ALL;
        Dictionary<char, List<KeyValuePair<string, string>>> Dic_Grouped;
        public FastReplace()
        {
            var cmp = new WordMappingComparer();
            Dic_ALL = new SortedDictionary<string, string>(cmp);
            Dic_Grouped = Dic_ALL.OrderBy(x => x.Key, cmp).GroupBy(x => x.Key.First()).ToDictionary(x => x.Key, x => x.ToList());
        }
        public FastReplace(Dictionary<string, string> _dic)
        {
            var cmp = new WordMappingComparer();
            Dic_ALL = new SortedDictionary<string, string>(_dic, cmp);
            Dic_Grouped = Dic_ALL.OrderBy(x => x.Key, cmp).GroupBy(x => x.Key.First()).ToDictionary(x => x.Key, x => x.ToList());
        }
        public FastReplace(SortedDictionary<string, string> _dic)
        {
            var cmp = new WordMappingComparer();
            Dic_ALL = _dic;
            Dic_Grouped = Dic_ALL.OrderBy(x => x.Key, cmp).GroupBy(x => x.Key.First()).ToDictionary(x => x.Key, x => x.ToList());
        }
        internal class WordMappingComparer : IComparer<string>
        {
            public int Compare(string x, string y)
            {
                // 越長的字串排在越前面，字串長度相等者，則採用字串預設的比序。
                if (x.Length == y.Length)
                {
                    return x.CompareTo(y);
                }
                return y.Length - x.Length;
            }
        }
        public string ReplaceAll(string source)
        {
            StringBuilder sb = new StringBuilder(source.Length);
            char[] key = Dic_Grouped.Keys.ToArray();
            int indexPass = 0, indexNow = 0;
            indexPass = indexNow;
            indexNow = source.IndexOfAny(key, indexNow);
            while (indexNow != -1)
            {

                sb.Append(source.Substring(indexPass, indexNow - indexPass));
                var _dic = Dic_Grouped[source[indexNow]];
                bool replaced = false;
                foreach (var a in _dic)
                {
                    if (indexNow + a.Key.Length > source.Length)
                        continue;
                    if (source.Substring(indexNow, a.Key.Length) == a.Key)
                    {
                        sb.Append(a.Value);
                        indexNow += a.Key.Length;
                        replaced = true;
                    }
                }
                if (!replaced)
                {
                    sb.Append(source.Substring(indexNow, 1));
                    indexNow++;
                }
                indexPass = indexNow;
                indexNow = source.IndexOfAny(key, indexNow);
            }
            sb.Append(source.Substring(indexPass, source.Length - indexPass));
            return sb.ToString();

        }
    }
}
