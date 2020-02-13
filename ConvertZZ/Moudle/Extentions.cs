using System;
using System.Collections.Generic;

namespace ConvertZZ.Moudle
{
    public static class Extentions
    {
        public static void AddRange<T1, T2>(this Dictionary<T1, T2> Dic, Dictionary<T1, T2> collection, bool Replace = true)
        {
            if (collection == null)
            {
                throw new ArgumentNullException("collection");
            }
            foreach (KeyValuePair<T1, T2> item in collection)
            {
                if (Dic.ContainsKey(item.Key))
                {
                    if (Replace)
                    {
                        Dic[item.Key] = item.Value;
                    }
                }
                else
                {
                    Dic.Add(item.Key, item.Value);
                }
            }
        }

        public static void RemoveRange<T1, T2>(this Dictionary<T1, T2> dic, Dictionary<T1, T2> collection)
        {
            if (collection == null)
            {
                throw new ArgumentNullException("collection");
            }
            foreach (KeyValuePair<T1, T2> item in collection)
            {
                if (dic.ContainsKey(item.Key))
                {
                    dic.Remove(item.Key);
                }
            }
        }
    }
}
