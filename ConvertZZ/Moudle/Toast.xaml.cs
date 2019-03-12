using System;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media.Animation;

namespace ConvertZZ.Moudle
{
    /// <summary>
    /// Toast.xaml 的互動邏輯
    /// </summary>
    public partial class Toast : Window, INotifyPropertyChanged
    {
        public Toast(string Message, int time_ms = 1000)
        {
            InitializeComponent();
            DataContext = this;
            this.Message = Message;
            this.time_ms = time_ms;
        }
        private async void Window_Loaded(object sender, RoutedEventArgs e)
        {
            DoubleAnimation daV = new DoubleAnimation(1, 0, new Duration(TimeSpan.FromMilliseconds(1000)));
            await Task.Delay(time_ms);
            BeginAnimation(UIElement.OpacityProperty, daV);
            await Task.Delay(1000);
            Close();
        }
        private void Window_MouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.LeftButton == MouseButtonState.Pressed)
            {
                DragMove();
            }
        }

        int time_ms = 1000;
        private string _Message;
        public string Message { get => _Message; set { _Message = value; OnPropertyChanged(); } }


        public event PropertyChangedEventHandler PropertyChanged;
        protected virtual void OnPropertyChanged([CallerMemberName] string propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }
    }
}
